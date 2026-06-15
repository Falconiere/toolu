#!/usr/bin/env bats
# Tests for credential discovery in lib/http.sh — reuse of an installed
# ankitpokhrel jira-cli (config + OS keyring), env precedence, and the friendly
# no-credentials message. The CLI config path and keyring binary are stubbed in
# the sandbox so tests never touch the host's real jira-cli setup.

load helpers

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

# Write a jira-cli config into the sandbox and point discovery at it.
_write_cli_config() { # server login installation
  local d="$SANDBOX/.jira"
  mkdir -p "$d"
  printf 'server: %s\nlogin: %s\ninstallation: %s\n' "$1" "$2" "$3" > "$d/.config.yml"
  export JIRA_CLI_CONFIG="$d/.config.yml"
}

@test "creds: reuses jira-cli server+login, token from JIRA_API_TOKEN env" {
  _write_cli_config "https://cli.atlassian.net" "cli@x.com" "Cloud"
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL
  export JIRA_API_TOKEN="envtok"
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://cli.atlassian.net/rest/api/3/myself' "$CURL_LOG"
  grep -qx -- '-u' "$CURL_LOG"
  grep -qx 'cli@x.com:envtok' "$CURL_LOG"
}

@test "creds: reuses api_token from the jira-cli config file" {
  local d="$SANDBOX/.jira"; mkdir -p "$d"
  printf 'server: https://cli.atlassian.net\nlogin: cfg@x.com\ninstallation: Cloud\napi_token: cfgtok\n' > "$d/.config.yml"
  export JIRA_CLI_CONFIG="$d/.config.yml"
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL JIRA_API_TOKEN
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'cfg@x.com:cfgtok' "$CURL_LOG"
}

@test "creds: reads the API token from ~/.netrc when config/env lack it" {
  _write_cli_config "https://gigpro.atlassian.net" "nr@x.com" "Cloud"
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL JIRA_API_TOKEN
  printf 'machine gigpro.atlassian.net login nr@x.com password netrctok\n' > "$SANDBOX/netrc"
  export NETRC="$SANDBOX/netrc"
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'nr@x.com:netrctok' "$CURL_LOG"
}

@test "creds: reads the API token from the OS keyring when env lacks it" {
  _write_cli_config "https://cli.atlassian.net" "kr@x.com" "Cloud"
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL JIRA_API_TOKEN
  cat > "$SANDBOX/bin/security" <<'SEC'
#!/usr/bin/env bash
printf 'keyringtok\n'
SEC
  chmod +x "$SANDBOX/bin/security"
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'kr@x.com:keyringtok' "$CURL_LOG"
}

@test "creds: installation Cloud selects api version 3" {
  _write_cli_config "https://cli.atlassian.net" "c@x.com" "Cloud"
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL JIRA_API_VERSION
  export JIRA_API_TOKEN=t
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && echo "VER=$_JIRA_VER"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"VER=3"* ]]
}

@test "creds: installation Local selects api version 2" {
  _write_cli_config "https://jira.acme.internal" "c@x.com" "Local"
  unset JIRA_BASE_URL JIRA_EMAIL JIRA_API_TOKEN JIRA_API_VERSION
  export JIRA_PAT=p
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && echo "VER=$_JIRA_VER"' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"VER=2"* ]]
}

@test "creds: explicit env overrides the jira-cli config" {
  _write_cli_config "https://cli.atlassian.net" "cli@x.com" "Cloud"
  unset JIRA_EMAIL JIRA_API_TOKEN
  export JIRA_BASE_URL="https://env.atlassian.net" JIRA_PAT="envpat"
  run bash -c 'source "$1/lib/http.sh"; jira_require_env && jira_curl GET /rest/api/3/myself' _ "$TOOL_DIR"
  [ "$status" -eq 0 ]
  grep -qx 'https://env.atlassian.net/rest/api/3/myself' "$CURL_LOG"
  grep -qx 'Authorization: Bearer envpat' "$CURL_LOG"
  run grep -q 'cli.atlassian.net' "$CURL_LOG"
  [ "$status" -ne 0 ]
}

@test "creds: no credentials prints a friendly setup message (not an error) and makes no request" {
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL JIRA_API_TOKEN
  export JIRA_CLI_CONFIG=/nonexistent/.config.yml
  run bash -c 'source "$1/lib/http.sh"; jira_require_env' _ "$TOOL_DIR"
  [ "$status" -eq 1 ]
  [[ "$output" == *"jira init"* ]]
  [[ "$output" == *"one-time setup step"* ]]
  [[ "$output" == *"Nothing is broken"* ]]
  [ ! -s "$CURL_LOG" ]
}
