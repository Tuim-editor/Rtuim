#!/usr/bin/env python3
"""
Tuim bug-report gateway - GitHub Issues backend.

A tiny HTTP server that receives the JSON payload Tuim's bug-report widget
POSTs and creates a GitHub issue on Tuim-editor/Rtuim, returning the shape the
widget expects:

    POST /bug-report
    -> 201 {"issueUrl": "https://github.com/Tuim-editor/Rtuim/issues/<n>"}

Run it alongside Tuim and set:

    TUIM_BUG_REPORT_ENDPOINT=http://127.0.0.1:9876/bug-report
    TUIM_BUG_REPORT_HOST=127.0.0.1
    TUIM_BUG_REPORT_PORT=9876

or pass -Dbug-report-endpoint=... at build time.
"""

import json
import os
import socket
import subprocess
from http.server import HTTPServer, BaseHTTPRequestHandler

REPO = "Tuim-editor/Rtuim"
PORT = int(os.environ.get("TUIM_BUG_REPORT_PORT", "9876"))
HOST = os.environ.get("TUIM_BUG_REPORT_HOST", "127.0.0.1")


def gh_token():
    try:
        out = subprocess.check_output(["gh", "auth", "token"], text=True)
        return out.strip()
    except Exception:
        return ""


def create_issue(payload):
    category = payload.get("category", "Other")
    summary = payload.get("summary", "")
    description = payload.get("description", "")
    metadata = payload.get("metadata", {})

    title = f"[{category}] {summary}".strip()
    body = (
        f"**Summary**\n{summary}\n\n"
        f"**Description**\n{description}\n\n"
        f"---\n\n**Environment**\n{json.dumps(metadata, indent=2)}"
    )

    body_json = json.dumps({"title": title, "body": body, "labels": [category]})

    try:
        proc = subprocess.run(
            [
                "gh", "api",
                "--method", "POST",
                "--header", "Accept: application/vnd.github+json",
                "--input", "-",
                f"/repos/{REPO}/issues",
            ],
            input=body_json,
            capture_output=True,
            text=True,
            timeout=30,
            env={**os.environ, "GH_TOKEN": gh_token()},
        )
        if proc.returncode == 0:
            resp = json.loads(proc.stdout)
            return 201, {"issueUrl": resp.get("html_url", "")}
        else:
            return 502, {}
    except (subprocess.TimeoutExpired, json.JSONDecodeError, KeyError):
        return 500, {}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass

    def do_POST(self):
        if self.path != "/bug-report":
            self.send_response(404)
            self.end_headers()
            return

        try:
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length)
            payload = json.loads(raw)
        except (ValueError, OSError):
            self.send_response(400)
            self.end_headers()
            return

        status, body = create_issue(payload)
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        if body:
            self.wfile.write(json.dumps(body).encode())


def pick_port(port):
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        try:
            s.bind((HOST, port))
            return port
        except OSError:
            for p in range(port + 1, port + 200):
                try:
                    s.bind((HOST, p))
                    return p
                except OSError:
                    continue
            raise RuntimeError(f"Cannot bind to {HOST}:{port} or any higher port")


def main():
    port = pick_port(PORT)
    server = HTTPServer((HOST, port), Handler)
    print(f"bug-report gateway listening on http://{HOST}:{port}/bug-report", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
