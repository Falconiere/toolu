#!/usr/bin/env bats
# Tests for lib/attachment.sh — issue attachments: add, list, get.

load helpers

setup() { setup_sandbox; export JIRA_PAT=tok; }
teardown() { teardown_sandbox; }

@test "attachment: add uploads the file multipart to the issue endpoint" {
  printf 'hi' > "$SANDBOX/up.txt"
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment add ABC-1 "$2"' _ "$TOOL_DIR" "$SANDBOX/up.txt"
  [ "$status" -eq 0 ]
  grep -qx -- '-F' "$CURL_LOG"
  grep -qx "file=@$SANDBOX/up.txt" "$CURL_LOG"
  grep -qx 'X-Atlassian-Token: no-check' "$CURL_LOG"
  grep -qx 'Authorization: Bearer tok' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1/attachments' "$CURL_LOG"
}

@test "attachment: add with a nonexistent file exits 1" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment add ABC-1 "$2"' _ "$TOOL_DIR" "$SANDBOX/missing.txt"
  [ "$status" -eq 1 ]
  [[ "$output" == *"file not found"* ]]
  run grep -q -- '-F' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "attachment: list hits the issue endpoint with the attachment field" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment list ABC-1' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/issue/ABC-1?fields=attachment' "$CURL_LOG"
}

@test "attachment: get hits the attachment metadata endpoint" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment get 10010' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://acme.atlassian.net/rest/api/3/attachment/10010' "$CURL_LOG"
}

@test "attachment: download saves content with auth and redirect-follow" {
  stub_responses attachment-text-body.txt
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment download 10010 -o "$2"' _ "$TOOL_DIR" "$SANDBOX/out.bin"
  [ "$status" -eq 0 ]
  grep -qx -- '-L' "$CURL_LOG"
  grep -qx 'https://acme.atlassian.net/rest/api/3/attachment/content/10010' "$CURL_LOG"
  grep -qx 'Authorization: Bearer tok' "$CURL_LOG"
  grep -q 'Deploy steps:' "$SANDBOX/out.bin"
}

@test "attachment: download without -o derives the filename from metadata" {
  stub_responses attachment-text.json attachment-text-body.txt
  run bash -c 'cd "$2"; source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment download 10010' _ "$TOOL_DIR" "$SANDBOX"
  [ "$status" -eq 0 ]
  [ -f "$SANDBOX/deploy-notes.txt" ]
  grep -q 'Deploy steps:' "$SANDBOX/deploy-notes.txt"
}

@test "attachment: read prints text content as context" {
  stub_responses attachment-text.json attachment-text-body.txt
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment read 10010' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"# deploy-notes.txt (text/plain)"* ]]
  [[ "$output" == *"Deploy steps:"* ]]
}

@test "attachment: read reports binary content instead of dumping bytes" {
  stub_responses attachment-image.json attachment-text-body.txt
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment read 10011' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"binary attachment (image/png"* ]]
  [[ "$output" == *"architecture.png"* ]]
  run grep -q 'Deploy steps:' <<< "$output"
  [ "$status" -ne 0 ]
}

@test "attachment: unknown action exits 1 with usage" {
  run bash -c 'source "$1/lib/http.sh"; source "$1/lib/attachment.sh"; jira_require_env && jira_attachment bogus' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira attachment"* ]]
}
