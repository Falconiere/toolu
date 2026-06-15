#!/usr/bin/env bash
# Shared bats helpers for the jira CLI tests.
#
# Each test gets a fresh sandbox holding the whole scripts/ tree (jira.sh + lib/)
# and a `curl` stub on PATH that records its argv to curl.log instead of hitting
# the network. The stub serves queued fixture bodies (recorded REAL Jira
# responses under fixtures/) one per call, or `{}` when none are queued. Tests
# assert against curl.log and the stub output. Credentials come from JIRA_* env
# vars — never a .env file.

# helpers.bash lives in scripts/__tests__/; the scripts dir is one level up.
SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="$SCRIPTS_DIR/__tests__/fixtures"

setup_sandbox() {
  SANDBOX="$(mktemp -d)"
  export SANDBOX FIXTURES SCRIPTS_DIR
  # Real PATH captured before the stub is prepended, so live-gated tests can
  # reach the real curl/binaries.
  export REAL_PATH="$PATH"
  export CURL_LOG="$SANDBOX/curl.log"
  export TOOL_DIR="$SANDBOX/jira"
  export JIRA_STUB_COUNTER="$SANDBOX/stub.counter"

  mkdir -p "$TOOL_DIR" "$SANDBOX/bin"
  [ -f "$SCRIPTS_DIR/jira.sh" ] && cp "$SCRIPTS_DIR/jira.sh" "$TOOL_DIR/jira.sh" && chmod +x "$TOOL_DIR/jira.sh"
  cp -R "$SCRIPTS_DIR/lib" "$TOOL_DIR/lib"

  cat > "$SANDBOX/bin/curl" <<'CURL'
#!/usr/bin/env bash
printf '%s\n' "$@" >> "$CURL_LOG"
if [[ -n "${JIRA_STUB_FAIL:-}" ]]; then
  printf '{"errorMessages":["stub failure"]}\n'
  exit "$JIRA_STUB_FAIL"
fi
# Resolve an -o/--output target (download), if present.
_out=""; _prev=""
for _a in "$@"; do
  [[ "$_prev" == "-o" || "$_prev" == "--output" ]] && _out="$_a"
  _prev="$_a"
done
# Body = next queued fixture (one per call), else {}.
if [[ -n "${JIRA_STUB_RESPONSES:-}" ]]; then
  mapfile -t _resp <<< "$JIRA_STUB_RESPONSES"
  _n=$(cat "$JIRA_STUB_COUNTER" 2>/dev/null || echo 0)
  _i=$_n
  (( _i >= ${#_resp[@]} )) && _i=$(( ${#_resp[@]} - 1 ))
  printf '%s\n' "$(( _n + 1 ))" > "$JIRA_STUB_COUNTER"
  _body=$(cat "${_resp[$_i]}")
else
  _body='{}'
fi
if [[ -n "$_out" ]]; then printf '%s' "$_body" > "$_out"; else printf '%s\n' "$_body"; fi
CURL
  chmod +x "$SANDBOX/bin/curl"
  export PATH="$SANDBOX/bin:$PATH"

  # Default base URL; auth + version are set per-test. Point CLI-cred discovery
  # at nothing so tests never read the host's real jira-cli config or keyring.
  export JIRA_BASE_URL="https://acme.atlassian.net"
  export JIRA_CLI_CONFIG=/dev/null
  export NETRC=/dev/null
}

teardown_sandbox() {
  [[ -n "${SANDBOX:-}" && -d "$SANDBOX" ]] && rm -rf "$SANDBOX"
  unset JIRA_BASE_URL JIRA_PAT JIRA_EMAIL JIRA_API_TOKEN JIRA_API_VERSION
  unset _JIRA_VER _JIRA_LEAN _JIRA_AUTH_MODE _JIRA_BASE JIRA_STUB_RESPONSES JIRA_STUB_FAIL REAL_PATH
  unset JIRA_CLI_CONFIG JIRA_KEYRING_SERVICE NETRC
}

# stub_responses f1 [f2 ...] — queue fixture bodies for successive curl calls.
# Each name is resolved against fixtures/ unless absolute. Resets the counter.
stub_responses() {
  local out="" f
  for f in "$@"; do
    [[ "$f" = /* ]] || f="$FIXTURES/$f"
    out+="$f"$'\n'
  done
  export JIRA_STUB_RESPONSES="${out%$'\n'}"
  printf '0\n' > "$JIRA_STUB_COUNTER"
}
