#!/usr/bin/env bats
# agent-browser.sh shapes token-lean defaults onto the read-heavy command
# family and passes everything else through untouched. The external
# `agent-browser` binary is replaced by a recording stub that logs its argv to
# $AB_LOG (mirrors exa-search's $CURL_LOG curl stub); the wrapper itself runs
# for real. $AGENT_BROWSER_BIN points the wrapper at the stub — no PATH surgery.

# `run -127` (asserting the wrapper's exit code) is a 1.5.0+ flag.
bats_require_minimum_version 1.5.0

setup() {
  TMP=$(mktemp -d)
  export AB_LOG="$TMP/argv.log"
  : > "$AB_LOG"
  cat > "$TMP/agent-browser" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$AB_LOG"
exit 0
EOF
  chmod +x "$TMP/agent-browser"
  export AGENT_BROWSER_BIN="$TMP/agent-browser"
  WRAP="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/agent-browser.sh"
}

teardown() { rm -rf "$TMP"; }

@test "snapshot: injects -i --json --max-output 4000 --content-boundaries" {
  run bash "$WRAP" snapshot
  [ "$status" -eq 0 ]
  line="$(cat "$AB_LOG")"
  [ "$line" = "snapshot -i --json --max-output 4000 --content-boundaries" ]
}

@test "snapshot --max-output 500: caller value kept, 4000 not added" {
  run bash "$WRAP" snapshot --max-output 500
  [ "$status" -eq 0 ]
  line="$(cat "$AB_LOG")"
  [ "$line" = "snapshot -i --json --content-boundaries --max-output 500" ]
}

@test "snapshot -a: caller scope kept, -i NOT added" {
  run bash "$WRAP" snapshot -a
  [ "$status" -eq 0 ]
  line="$(cat "$AB_LOG")"
  [ "$line" = "snapshot --json --max-output 4000 --content-boundaries -a" ]
}

@test "screenshot: passthrough, no injected flags" {
  run bash "$WRAP" screenshot page.png
  [ "$status" -eq 0 ]
  [ "$(cat "$AB_LOG")" = "screenshot page.png" ]
}

@test "click: passthrough, no injected flags" {
  run bash "$WRAP" click @e2
  [ "$status" -eq 0 ]
  [ "$(cat "$AB_LOG")" = "click @e2" ]
}

@test "--raw snapshot: total bypass, nothing injected" {
  run bash "$WRAP" --raw snapshot
  [ "$status" -eq 0 ]
  [ "$(cat "$AB_LOG")" = "snapshot" ]
}

@test "unknown subcommand: passthrough, nothing injected" {
  run bash "$WRAP" wibble x
  [ "$status" -eq 0 ]
  [ "$(cat "$AB_LOG")" = "wibble x" ]
}

@test "binary absent: install guide to stderr, exits 127" {
  run -127 bash -c 'AGENT_BROWSER_BIN="$1/absent" bash "$2" snapshot 2>&1' _ "$TMP" "$WRAP"
  [[ "$output" == *"agent-browser not found"* ]]
}
