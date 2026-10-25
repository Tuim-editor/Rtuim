#!/usr/bin/env bash
# Tuim bug-report gateway → GitHub Issues
#
# Tuim's bug-report widget POSTs a JSON payload via curl and expects:
#   HTTP 201 + {"issueUrl":"https://…"}
#
# This gateway receives that payload, creates a GitHub issue on
# Rouboufy/tuim with a label matching the report category, and returns
# the required shape so the widget's poll() loop succeeds.
#
# Contract (matches the widget's curl invocation):
#   $1  → payload file path (the JSON Tuim wrote)
#   $2  → response file path (write {"issueUrl":"…"} here)
#   $3  → status file path   (write the HTTP status code here)
#
# Used by the bug-report widget's curl submit loop. Keep it fast and quiet.

set -euo pipefail

payload_file="${1:-}"
response_file="${2:-}"
status_file="${3:-}"

repo="Rouboufy/tuim"

# ---- read + normalize the posted payload ---------------------------------

if [ ! -s "$payload_file" ]; then
    printf '400' > "$status_file"
    exit 0
fi

payload="$(cat "$payload_file")"

category="$(printf '%s\n' "$payload" | python3 -c "import json,sys; print(json.load(sys.stdin).get('category','Other'))" 2>/dev/null || echo "Other")"
summary="$(printf '%s\n' "$payload" | python3 -c "import json,sys; print(json.load(sys.stdin).get('summary',''))" 2>/dev/null || echo "")"
description="$(printf '%s\n' "$payload" | python3 -c "import json,sys; print(json.load(sys.stdin).get('description',''))" 2>/dev/null || echo "")"
metadata_json="$(printf '%s\n' "$payload" | python3 -c "import json,sys; print(json.dumps(json.load(sys.stdin).get('metadata',{})))" 2>/dev/null || echo "{}")"

# ---- build the issue body -----------------------------------------------

title="$(printf '[%s] %s' "$category" "$summary" | sed 's/  */ /g' | sed 's/^ //;s/ $//')"

body="$(cat <<BODY
**Summary**
${summary}

**Description**
${description}

---

**Environment**
${metadata_json}
BODY
)"

# ---- build the JSON request body ----------------------------------------

request_body="$(python3 - "$title" "$body" "$category" <<'PY'
import json, sys
title, body, labels = sys.argv[1], sys.argv[2], sys.argv[3]
print(json.dumps({
    "title": title,
    "body": body,
    "labels": [labels],
}))
PY
)"

# ---- create the issue (single API call, capture status + url) ----------

token="$(gh auth token 2>/dev/null)" || {
    printf '502' > "$status_file"
    exit 0
}

# -i includes the HTTP status line in stdout; -q jq extracts the url to
# stderr is not how -q works, so we capture stdout (headers+body) and split.
full_output="$(printf '%s\n' "$request_body" | \
    gh api \
        --method POST \
        --header "Authorization: token $token" \
        --header "Accept: application/vnd.github+json" \
        --input - \
        -i \
        "/repos/$repo/issues" 2>/dev/null)" || {
    printf '502' > "$status_file"
    exit 0
}

http_status="$(printf '%s\n' "$full_output" | tr -d '\r' | sed -n 's/^HTTP\/[0-9.]* \([0-9][0-9][0-9]\).*/\1/p' | head -n1)"
body_output="$(printf '%s\n' "$full_output" | tr -d '\r' | sed -n '/^$/,$p' | tail -n +2)"

printf '%s' "$http_status" > "$status_file"

if [ "$http_status" = "201" ]; then
    issue_url="$(printf '%s\n' "$body_output" | python3 -c "import json,sys; print(json.load(sys.stdin)['html_url'])" 2>/dev/null || true)"
    if [ -n "$issue_url" ]; then
        printf '{"issueUrl":"%s"}\n' "$issue_url" > "$response_file"
    else
        printf '500' > "$status_file"
        : > "$response_file"
    fi
else
    : > "$response_file"
fi
