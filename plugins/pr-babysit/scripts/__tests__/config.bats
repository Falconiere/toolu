#!/usr/bin/env bats
# Real-data tests for scripts/lib/config.sh (AC-9): real toolu.config.json
# files under temp config/project dirs, and path parity against the real
# toolu host adapter (plugins/toolu/hooks/lib/host.sh). No mocks.

LIB="${BATS_TEST_DIRNAME}/../lib"
TOOLU_HOST_LIB="$(cd "${BATS_TEST_DIRNAME}/../../../toolu/hooks/lib" && pwd -P)/host.sh"

setup() {
  TMP=$(mktemp -d)
  mkdir -p "$TMP/user" "$TMP/project/.claude" "$TMP/project/.codex"
  # Every test starts from a clean environment: no host or override leaks in.
  unset TOOLU_CONFIG_DIR TOOLU_PROJECT_DIR TOOLU_PROJECT_CONFIG_DIRNAME CLAUDE_CONFIG_DIR CODEX_HOME CLAUDE_PROJECT_DIR PLUGIN_ROOT TOOLU_HOST_OVERRIDE
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

with_config() {
  bash -c "set -euo pipefail; . '$LIB/common.sh'; . '$LIB/hosts.sh'; . '$LIB/config.sh'; $1"
}

# toolu_paths HOST -> "<user>|<project>" from toolu's own host adapter
toolu_paths() {
  TOOLU_HOST_OVERRIDE="$1" bash -c ". '$TOOLU_HOST_LIB'; printf '%s|%s' \"\$(toolu_config_root)/toolu.config.json\" \"\$(toolu_project_config)\""
}

@test "AC-9: TOOLU_CONFIG_DIR / TOOLU_PROJECT_DIR resolve the same paths as toolu host.sh (claude, codex)" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  for host in claude codex; do
    got=$(with_config "printf '%s|%s' \"\$(pb_config_user_path $host)\" \"\$(pb_config_project_path $host)\"")
    [ "$got" = "$(toolu_paths "$host")" ]
  done
  [ "$(with_config 'pb_config_project_path codex')" = "$TMP/project/.codex/toolu.config.json" ]
}

@test "AC-9: CLAUDE_CONFIG_DIR / CODEX_HOME and TOOLU_PROJECT_CONFIG_DIRNAME keep parity" {
  export CLAUDE_CONFIG_DIR="$TMP/claude-home" CODEX_HOME="$TMP/codex-home" TOOLU_PROJECT_DIR="$TMP/project"
  for host in claude codex; do
    got=$(with_config "printf '%s|%s' \"\$(pb_config_user_path $host)\" \"\$(pb_config_project_path $host)\"")
    [ "$got" = "$(toolu_paths "$host")" ]
  done
  export TOOLU_PROJECT_CONFIG_DIRNAME=.toolu-cfg
  [ "$(with_config 'pb_config_project_path claude')" = "$(toolu_paths claude | cut -d'|' -f2)" ]
}

@test "AC-9: project values override user values; user-only keys survive" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  echo '{"prBabysit":{"hosts":["codex"],"unattended":false}}' >"$TMP/user/toolu.config.json"
  echo '{"prBabysit":{"hosts":["cursor-agent"]}}' >"$TMP/project/.claude/toolu.config.json"
  out=$(with_config 'pb_config_load claude')
  [ "$(jq -c '.hosts' <<<"$out")" = '["cursor"]' ]
  [ "$(jq -r '.unattended' <<<"$out")" = false ]
  [ "$(jq -r '.dispatch' <<<"$out")" = herdr ]
}

@test "AC-9: no config files -> defaults with the controller host as the pool" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  out=$(with_config 'pb_config_load codex')
  [ "$(jq -c '{dispatch, hosts, prefer, unattended, jev}' <<<"$out")" = '{"dispatch":"herdr","hosts":["codex"],"prefer":{},"unattended":true,"jev":true}' ]
  [ "$(jq -S -c '.routing' <<<"$out")" = "$(with_config 'pb_default_routing_json' | jq -S -c .)" ]
}

@test "AC-9: a routing override replaces only that host's row" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  echo '{"prBabysit":{"routing":{"codex":[{"model":"gpt-6-luna","effort":"low"},{"model":"gpt-6-luna","effort":"medium"},{"model":"gpt-6-sol","effort":"high"},{"model":"gpt-6-sol","effort":"xhigh"}]}}}' >"$TMP/project/.claude/toolu.config.json"
  out=$(with_config 'pb_config_load claude')
  [ "$(jq -r '.routing.codex[0].model' <<<"$out")" = gpt-6-luna ]
  [ "$(jq -r '.routing.claude[3].model' <<<"$out")" = opus ]
}

@test "AC-9 boundary: malformed project JSON -> user values + defaults and exactly one stderr warning" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  echo '{"prBabysit":{"hosts":["codex"]}}' >"$TMP/user/toolu.config.json"
  printf '{"prBabysit": {' >"$TMP/project/.claude/toolu.config.json"
  out=$(with_config 'pb_config_load claude' 2>"$TMP/err")
  [ "$(jq -c '.hosts' <<<"$out")" = '["codex"]' ]
  [ "$(wc -l <"$TMP/err" | tr -d ' ')" -eq 1 ]
  grep -q 'malformed JSON' "$TMP/err"
}

@test "AC-9 boundary: invalid values are config_invalid (exit 3)" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  for bad in '{"dispatch":"sometimes"}' '{"hosts":["gemini"]}' '{"hosts":[]}' '{"prefer":{"urgent":["claude"]}}' \
             '{"routing":{"codex":[{"model":"a b"},{},{},{}]}}' '{"routing":{"codex":[{}]}}' '{"unattended":"yes"}' '{"jev":1}'; do
    jq -n --argjson v "$bad" '{prBabysit:$v}' >"$TMP/project/.claude/toolu.config.json"
    run with_config 'pb_config_load claude'
    [ "$status" -eq 3 ] || { echo "accepted: $bad" >&2; return 1; }
    [ "$(jq -r '.errors[0].code' <<<"$output")" = config_invalid ]
  done
}

@test "AC-9 boundary: a prBabysit that is not an object is config_invalid naming the file, also through route-fix.sh" {
  export TOOLU_CONFIG_DIR="$TMP/user" TOOLU_PROJECT_DIR="$TMP/project"
  for bad in '"x"' '[]' 'false'; do
    jq -n --argjson v "$bad" '{prBabysit:$v}' >"$TMP/project/.claude/toolu.config.json"
    run with_config 'pb_config_load claude'
    [ "$status" -eq 3 ] || { echo "accepted: $bad" >&2; return 1; }
    [ "$(jq -c '.errors[0] | [.code, .file]' <<<"$output")" = "[\"config_invalid\",\"$TMP/project/.claude/toolu.config.json\"]" ]
  done
  run bash "${BATS_TEST_DIRNAME}/../route-fix.sh" --items "${BATS_TEST_DIRNAME}/fixtures/items/review-items.json" --host claude --no-jev
  [ "$status" -eq 3 ]
  [ "$(jq -r '.errors[0].message' <<<"$output")" = "prBabysit must be an object (in $TMP/project/.claude/toolu.config.json)" ]
}

@test "pb_config_keys lists exactly the keys the reader accepts" {
  [ "$(with_config 'pb_config_keys' | tr '\n' ' ')" = "dispatch hosts prefer routing unattended jev " ]
}
