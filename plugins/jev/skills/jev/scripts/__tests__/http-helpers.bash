#!/usr/bin/env bash
# Real curl, redirected to a private HTTPS listener via its native config.
setup_http() {
  SANDBOX="$(mktemp -d)"
  export CURL_HOME="$SANDBOX"
  export TYPESAFE_API_KEY=test-typesafe-key-123
  TOOL_DIR="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
  export TOOL_DIR
  CURL_LOG="$SANDBOX/requests.jsonl"
  openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
    -keyout "$SANDBOX/key.pem" -out "$SANDBOX/cert.pem" -days 1 \
    -subj /CN=api.typesafe.ai >/dev/null 2>&1
  python3 "$BATS_TEST_DIRNAME/http-server.py" "$SANDBOX" >"$SANDBOX/server.log" 2>&1 &
  SERVER_PID=$!
  for _ in {1..100}; do
    [ -s "$SANDBOX/port" ] && break
    sleep 0.02
  done
  [ -s "$SANDBOX/port" ] || { cat "$SANDBOX/server.log" >&2; return 1; }
  cat > "$SANDBOX/.curlrc" <<EOF
connect-to = "api.typesafe.ai:443:127.0.0.1:$(cat "$SANDBOX/port")"
insecure
noproxy = "*"
EOF
}

teardown_http() {
  kill "$SERVER_PID" 2>/dev/null || true
  wait "$SERVER_PID" 2>/dev/null || true
  rm -rf "$SANDBOX"
  unset CURL_HOME TYPESAFE_API_KEY JEV_TIMEOUT
}

response_body() { printf '%s' "$1" > "$SANDBOX/response.json"; }
body_json() { jq -sc '.[0].body' "$CURL_LOG"; }
