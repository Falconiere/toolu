#!/usr/bin/env bash
# Shared bats helpers for the context7 / exa-search search.sh script tests.
#
# Each test gets a fresh sandbox with a `curl` stub on PATH that records its
# argv to <TMP>/curl.log instead of hitting the network. Tests assert against
# curl.log to verify behavior. API keys are passed via the environment
# variables CONTEXT7_API_KEY / EXA_API_KEY — never via a .env file.
#
# Shared from tooling/ so context7 and exa-search keep ONE copy. SCRIPT_DIR is
# derived from BATS_TEST_DIRNAME (the consuming test's own scripts/__tests__),
# NOT this helper's location, so each plugin resolves its OWN scripts/search.sh.

SCRIPT_DIR="$(cd "${BATS_TEST_DIRNAME}/.." && pwd)"

setup_sandbox() {
  local tool="$1"
  SANDBOX="$(mktemp -d)"
  export SANDBOX
  export CURL_LOG="$SANDBOX/curl.log"
  export TOOL_DIR="$SANDBOX/$tool"

  mkdir -p "$TOOL_DIR" "$SANDBOX/bin"
  cp "$SCRIPT_DIR/search.sh" "$TOOL_DIR/search.sh"
  chmod +x "$TOOL_DIR/search.sh"

  cat > "$SANDBOX/bin/curl" <<'CURL'
#!/usr/bin/env bash
printf '%s\n' "$@" >> "$CURL_LOG"
# Emit a minimal JSON body so `jq '.'` downstream does not choke.
printf '{}\n'
CURL
  chmod +x "$SANDBOX/bin/curl"

  export PATH="$SANDBOX/bin:$PATH"
}

teardown_sandbox() {
  [[ -n "${SANDBOX:-}" && -d "$SANDBOX" ]] && rm -rf "$SANDBOX"
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
