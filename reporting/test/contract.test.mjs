import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { generateKeyPairSync, verify } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const build = mkdtempSync(join(tmpdir(), "tuim-report-test-"));
after(() => rmSync(build, { recursive: true, force: true }));
// Compile the actual Worker, rather than testing a separate validation implementation.
execFileSync(process.execPath, [join(root, "node_modules/typescript/bin/tsc"),
  "--project", join(root, "tsconfig.json"), "--noEmit", "false", "--outDir", build],
{ stdio: "inherit" });
writeFileSync(join(build, "package.json"), '{"type":"module"}');

const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const privatePem = privateKey.export({ type: "pkcs1", format: "pem" });
const valid = { category: "Editor", summary: "Cursor jumps", description: "Steps to reproduce", diagnostics: '{"os":"linux"}' };
let moduleId = 0;

async function fixture(t, options = {}) {
  // Fresh module state isolates the installation-token cache between cases.
  const { default: worker } = await import(`${pathToFileURL(join(build, "index.js"))}?case=${moduleId++}`);
  const counts = new Map();
  const writes = [];
  const calls = [];
  const env = {
    GH_APP_ID: "1234", GH_INSTALLATION_ID: "5678", GH_PRIVATE_KEY: privatePem,
    GITHUB_OWNER: "test-owner", GITHUB_REPO: "test-repo",
    REPORT_LIMITS: {
      async get(key) { return counts.get(key) ?? null; },
      async put(key, value, settings) { counts.set(key, value); writes.push({ key, value, settings }); },
    },
  };
  t.mock.method(console, "error", () => {});
  // No fallback to real fetch: even unexpected requests cannot leave this process.
  t.mock.method(globalThis, "fetch", async (url, init) => {
    calls.push({ url, ...init, json: JSON.parse(init.body) });
    if (url === "https://api.github.com/app/installations/5678/access_tokens") {
      if (options.token) return options.token();
      return Response.json({ token: "server-only-token", expires_at: new Date(Date.now() + 3600_000).toISOString() });
    }
    assert.equal(url, "https://api.github.com/repos/test-owner/test-repo/issues");
    if (options.issue) return options.issue();
    return Response.json({ html_url: "https://github.com/test-owner/test-repo/issues/42", number: 42 }, { status: 201 });
  });
  const sendRaw = (body, headers = {}, method = "POST") => worker.fetch(new Request("https://gateway.invalid/report", {
    method, headers: { "Content-Type": "application/json", "CF-Connecting-IP": "192.0.2.1", ...headers },
    ...(method === "GET" ? {} : { body }),
  }), env);
  return { env, calls, counts, writes, sendRaw, send: (payload = valid, headers) => sendRaw(JSON.stringify(payload), headers) };
}

for (const category of ["Editor", "Terminal", "Git", "Extensions", "Performance", "Other"]) {
  test(`Rust contract accepts ${category} and returns issueUrl`, async (t) => {
    const { send, calls } = await fixture(t);
    const response = await send({ ...valid, category, summary: "  x  " });
    assert.equal(response.status, 201);
    assert.equal(response.headers.get("Cache-Control"), "no-store");
    assert.deepEqual(await response.json(), { issueUrl: "https://github.com/test-owner/test-repo/issues/42", issueNumber: 42 });
    assert.equal(calls[1].json.title, `[${category}] x`);
    assert.ok(calls[1].json.body.includes(`### Category\n\n${category}`));
    assert.ok(calls[1].json.body.includes(valid.description));
    assert.ok(calls[1].json.body.includes(valid.diagnostics));
  });
}

for (const description of [undefined, "", " ", "x"]) {
  test(`description is optional: ${JSON.stringify(description)}`, async (t) => {
    const { send, calls } = await fixture(t);
    const response = await send({ ...valid, description, diagnostics: "" });
    assert.equal(response.status, 201);
    assert.ok(!calls[1].json.body.includes("undefined"));
    assert.equal(calls[1].json.body.includes("### Description"), Boolean(description));
  });
}

const invalid = [
  null, [], true, "report", 1, {},
  { ...valid, category: undefined }, { ...valid, category: "Source control" },
  { ...valid, category: "git" }, { ...valid, category: "" }, { ...valid, category: 1 },
  { ...valid, summary: undefined }, { ...valid, summary: null }, { ...valid, summary: 1 },
  { ...valid, summary: "" }, { ...valid, summary: " \n\t\u2003" },
  { ...valid, description: null }, { ...valid, description: {} }, { ...valid, description: 1 },
  { ...valid, diagnostics: undefined }, { ...valid, diagnostics: null },
  { ...valid, diagnostics: {} }, { ...valid, diagnostics: [] }, { ...valid, diagnostics: 1 },
  { category: "Editor", summary: "Old payload", description: "Old shape", metadata: {}, logs: null },
];
for (const [index, payload] of invalid.entries()) {
  test(`invalid payload ${index + 1} is rejected before GitHub`, async (t) => {
    const { send, calls } = await fixture(t);
    const response = await send(payload);
    assert.equal(response.status, 422);
    assert.deepEqual(await response.json(), { error: "Invalid report" });
    assert.equal(calls.length, 0);
  });
}

for (const [field, limit] of [["summary", 120], ["description", 6000], ["diagnostics", 40_000]]) {
  for (const char of ["a", "\u{1f680}"]) {
    test(`${field} counts Unicode code points: ${limit} accepted, ${limit + 1} rejected (${char})`, async (t) => {
      const { send, calls } = await fixture(t);
      const response = await send({ ...valid, [field]: char.repeat(limit) });
      assert.equal(response.status, 201);
      const issue = calls[1].json;
      assert.ok((field === "summary" ? issue.title : issue.body).includes(char.repeat(limit)));
      assert.equal((await send({ ...valid, [field]: char.repeat(limit + 1) })).status, 422);
      assert.equal(calls.length, 2);
    });
  }
}

for (const [char, escaped] of [["\uffff", "\\uffff"], ["\u{1f680}", "\\ud83d\\ude80"]]) {
  test(`all fields can reach their limits with JSON-escaped Unicode (${escaped})`, async (t) => {
    const { sendRaw } = await fixture(t);
    const payload = { category: "Other", summary: char.repeat(120), description: char.repeat(6000), diagnostics: char.repeat(40_000) };
    const raw = JSON.stringify(payload).replaceAll(char, escaped);
    assert.ok(Buffer.byteLength(raw) > 46_120 * escaped.length);
    assert.equal((await sendRaw(raw, { "Content-Length": String(Buffer.byteLength(raw)) })).status, 201);
  });
}

for (const diagnostics of [
  "plain diagnostics (not necessarily JSON)",
  "```\n# injected heading\n<script>alert(1)</script>\n``````\n~~~\n~~~~~~~~",
  "`".repeat(40_000),
  "~".repeat(40_000),
  "`".repeat(19_999) + "\n" + "~".repeat(20_000),
  "~~~\n".repeat(10_000),
]) {
  test(`diagnostics stay inside a safe Markdown fence (${diagnostics.length} chars)`, async (t) => {
    const { send, calls } = await fixture(t);
    assert.equal((await send({ ...valid, diagnostics })).status, 201);
    const body = calls[1].json.body;
    const section = body.split("### Diagnostics\n\n")[1];
    const opening = section.slice(0, section.indexOf("\n"));
    const fence = opening.slice(0, -4); // Strip the "text" info string.
    assert.match(fence, /^(?:`{3,}|~{3,})$/);
    const content = section.slice(opening.length + 1, section.lastIndexOf(`\n${fence}\n`));
    assert.doesNotMatch(content, /^ {0,3}~{3,}[ \t]*\r?$/m);
    assert.equal(content.replaceAll("\u200b", ""), diagnostics);
    assert.ok(body.length < 65_536);
  });
}

test("GitHub App signs a server-side JWT, scopes and caches installation tokens", async (t) => {
  const { send, calls } = await fixture(t);
  const response = await send();
  assert.equal(response.status, 201);
  assert.equal(calls[0].method, "POST");
  assert.deepEqual(calls[0].json, { repositories: ["test-repo"], permissions: { issues: "write" } });
  const jwt = calls[0].headers.Authorization.slice("Bearer ".length);
  const [header, payload, signature] = jwt.split(".");
  assert.deepEqual(JSON.parse(Buffer.from(header, "base64url")), { alg: "RS256", typ: "JWT" });
  const claims = JSON.parse(Buffer.from(payload, "base64url"));
  assert.equal(claims.iss, "1234");
  assert.ok(claims.exp > Date.now() / 1000 && claims.exp < Date.now() / 1000 + 600);
  assert.ok(verify("RSA-SHA256", Buffer.from(`${header}.${payload}`), publicKey, Buffer.from(signature, "base64url")));
  assert.equal(calls[1].headers.Authorization, "Bearer server-only-token");
  assert.equal(calls[1].headers["Content-Type"], "application/json");
  assert.ok(!(await response.text()).includes("server-only-token"));
  assert.equal((await send()).status, 201);
  assert.equal(calls.length, 3);
  assert.equal(calls[2].headers.Authorization, "Bearer server-only-token");
});

test("installation tokens near expiry are refreshed", async (t) => {
  const { send, calls } = await fixture(t, { token: () => Response.json({ token: "short-lived", expires_at: new Date(Date.now() + 30_000).toISOString() }) });
  assert.equal((await send()).status, 201);
  assert.equal((await send()).status, 201);
  assert.equal(calls.length, 4);
});

for (const phase of ["token", "issue"]) {
  for (const status of [401, 403, 422, 429, 500]) {
    test(`GitHub ${phase} HTTP ${status} returns a generic 502`, async (t) => {
      const { send, calls } = await fixture(t, { [phase]: () => Response.json({ message: "private upstream detail" }, { status }) });
      const response = await send();
      assert.equal(response.status, 502);
      assert.deepEqual(await response.json(), { error: "Issue creation failed" });
      assert.equal(calls.length, phase === "token" ? 1 : 2);
    });
  }
  for (const [name, handler] of [
    ["network failure", () => { throw new Error("private network detail"); }],
    ["invalid JSON", () => new Response("not json")],
    ["missing fields", () => Response.json({})],
    ["null", () => Response.json(null)],
    ["invalid fields", () => Response.json({ token: 1, expires_at: "invalid", html_url: "http://insecure.invalid", number: -1 })],
  ]) {
    test(`GitHub ${phase} ${name} returns a generic 502`, async (t) => {
      const { send } = await fixture(t, { [phase]: handler });
      const response = await send();
      assert.equal(response.status, 502);
      assert.deepEqual(await response.json(), { error: "Issue creation failed" });
    });
  }
}

test("five reports per IP/hour, with hashed KV keys and no user auth", async (t) => {
  const { send, calls, writes } = await fixture(t);
  for (let i = 0; i < 5; i++) assert.equal((await send()).status, 201);
  const blocked = await send();
  assert.equal(blocked.status, 429);
  assert.deepEqual(await blocked.json(), { error: "Too many reports; try again later" });
  assert.equal(calls.length, 6); // One token request and five issues.
  assert.equal(writes.length, 5);
  assert.match(writes[0].key, /^hour:\d{4}-\d\d-\d\dT\d\d:[\w-]+$/);
  assert.ok(!writes[0].key.includes("192.0.2.1"));
  assert.deepEqual(writes.map(({ value }) => value), ["1", "2", "3", "4", "5"]);
  assert.ok(writes.every(({ settings }) => settings.expirationTtl === 7200));
  assert.equal((await send(valid, { "CF-Connecting-IP": "192.0.2.2" })).status, 201);
});

test("failed GitHub requests also consume the abuse budget", async (t) => {
  const { send, calls } = await fixture(t, { token: () => new Response("", { status: 500 }) });
  for (let i = 0; i < 5; i++) assert.equal((await send()).status, 502);
  assert.equal((await send()).status, 429);
  assert.equal(calls.length, 5);
});

test("method, media type, malformed JSON and byte-size guards", async (t) => {
  const { sendRaw, calls } = await fixture(t);
  assert.equal((await sendRaw(undefined, {}, "GET")).status, 200);
  assert.equal((await sendRaw("", {}, "PUT")).status, 405);
  assert.equal((await sendRaw("{}", { "Content-Type": "text/plain" })).status, 415);
  assert.equal((await sendRaw("{}", { "Content-Length": "560001" })).status, 413);
  assert.equal((await sendRaw(" ".repeat(560_001))).status, 413);
  // The guard must measure UTF-8 bytes, not JavaScript string length.
  assert.equal((await sendRaw("\u{1f680}".repeat(140_001))).status, 413);
  assert.equal((await sendRaw("{")).status, 400);
  assert.equal(calls.length, 0);
});

test("JSON content type may include a charset", async (t) => {
  const { send } = await fixture(t);
  assert.equal((await send(valid, { "Content-Type": "application/json; charset=utf-8" })).status, 201);
});
