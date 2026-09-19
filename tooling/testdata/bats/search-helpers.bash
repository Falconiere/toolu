#!/usr/bin/env bash
# Shared bats helpers for the REST-wrapper script tests (context7, exa-search, jev).
#
# Each test gets a fresh sandbox with a `curl` stub on PATH that records its
# argv to <TMP>/curl.log instead of hitting the network. Tests assert against
# curl.log to verify behavior. API keys are passed via the environment
# variables CONTEXT7_API_KEY / EXA_API_KEY / TYPESAFE_API_KEY — never via a
# .env file.
#
# Shared from tooling/ so every wrapper plugin keeps ONE copy. SCRIPT_DIR is
# derived from BATS_TEST_DIRNAME (the consuming test's own scripts/__tests__),
# NOT this helper's location, so each plugin resolves its OWN script.
#
# The stub replies `{}` by default. Set CURL_STUB_BODY to a real API response
# body when a test needs to assert on what the script PRINTS rather than on
# what it sent.

SCRIPT_DIR="$(cd "${BATS_TEST_DIRNAME}/.." && pwd)"

# setup_sandbox TOOL [SCRIPT_NAME]
# SCRIPT_NAME defaults to search.sh, so existing callers stay unchanged.
setup_sandbox() {
  local tool="$1"
  local script="${2:-search.sh}"
  SANDBOX="$(mktemp -d)"
  export SANDBOX
  export CURL_LOG="$SANDBOX/curl.log"
  export TOOL_DIR="$SANDBOX/$tool"

  mkdir -p "$TOOL_DIR" "$SANDBOX/bin"
  cp "$SCRIPT_DIR/$script" "$TOOL_DIR/$script"
  chmod +x "$TOOL_DIR/$script"

  cat > "$SANDBOX/bin/curl" <<'CURL'
#!/usr/bin/env bash
printf '%s\n' "$@" >> "$CURL_LOG"
# Emit a response body so `jq` downstream does not choke. A test that asserts
# on printed output sets CURL_STUB_BODY to a real API response.
if [ -n "${CURL_STUB_BODY:-}" ]; then
  printf '%s\n' "$CURL_STUB_BODY"
else
  printf '{}\n'
fi
CURL
  chmod +x "$SANDBOX/bin/curl"

  export PATH="$SANDBOX/bin:$PATH"
}

teardown_sandbox() {
  # Unset first and unconditionally: an early failure that leaves SANDBOX unset
  # must still clear the stub body, or it leaks into the next test.
  unset CURL_STUB_BODY
  if [[ -n "${SANDBOX:-}" && -d "$SANDBOX" ]]; then
    rm -rf "$SANDBOX"
  fi
}

# Set or unset an API key for the next script invocation.
# Usage: set_api_key CONTEXT7_API_KEY ctx7sk_abc123
#        set_api_key EXA_API_KEY ""        # explicit empty
set_api_key() {
  local name="$1"
  local value="${2:-}"
  if [ -z "$value" ]; then
    unset "$name"
  else
    export "$name=$value"
  fi
}
