#!/usr/bin/env bats
# Tests for the comemory plugin's comemory-scope.sh registry module.

HOOK="${BATS_TEST_DIRNAME}/../comemory-scope.sh"

# Core lib lives in the sibling toolu plugin; the dispatcher provides
# this env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "${BATS_TEST_DIRNAME}/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

_mk() {
  jq -n --arg c "$1" '{tool_name:"Bash", tool_input:{command:$c}}'
}

_decision() {
  if [[ -z "$1" ]]; then
    echo "allow"
    return
  fi
  echo "$1" | jq -r '.hookSpecificOutput.permissionDecision // "allow"' 2>/dev/null || echo "allow"
}

@test "comemory-scope: non-Bash tool exits silently" {
  payload=$(_mk 'comemory search foo')
  run bash -c "tool_name=Edit input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "comemory-scope: bare comemory search is denied" {
  payload=$(_mk 'comemory search "x"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: bare comemory save is denied" {
  payload=$(_mk 'comemory save body --kind note')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: bare comemory context is denied" {
  payload=$(_mk 'comemory context query')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: comemory search with --repo is allowed" {
  payload=$(_mk 'comemory search "x" --repo toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: comemory save with --repo is allowed" {
  payload=$(_mk 'comemory save body --kind note --repo toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: comemory search with --repo= form is allowed" {
  payload=$(_mk 'comemory search "x" --repo=toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: wrapper call is allowed (bypass)" {
  payload=$(_mk 'skills/agent-memory/scripts/comemory.sh search "x"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: bare comemory search-code is denied" {
  payload=$(_mk 'comemory search-code "fn foo"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: comemory search-code with --repo is allowed" {
  payload=$(_mk 'comemory search-code "fn foo" --repo toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: bare comemory index-code is denied" {
  payload=$(_mk 'comemory index-code --path /tmp/x')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: comemory index-code with --repo is allowed" {
  payload=$(_mk 'comemory index-code --path /tmp/x --repo toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: bare comemory graph is denied" {
  payload=$(_mk 'comemory graph --rel all')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: comemory graph with --repo is allowed" {
  payload=$(_mk 'comemory graph --rel all --repo toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: retrieval-loop verb (mine) is allowed (global by design)" {
  payload=$(_mk 'comemory mine --apply')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

# Regression: the wrapper-skip used to match the WHOLE command, so a single
# `comemory.sh` token short-circuited enforcement for every other segment.
# Per-segment skipping must still deny an unscoped raw call chained after a
# legitimate wrapper call.
@test "comemory-scope: wrapper call chained with a raw unscoped search is denied" {
  payload=$(_mk 'comemory.sh list && comemory search foo')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

# Regression: a mere mention of `comemory.sh` (in an echo / comment) must
# not disable enforcement for a real unscoped call in another segment.
@test "comemory-scope: comemory.sh token in echo does not excuse a later raw save" {
  payload=$(_mk 'echo using comemory.sh ; comemory save body --kind note')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: comemory list is allowed (global by design)" {
  payload=$(_mk 'comemory list --repo toolu')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: comemory doctor is allowed (global by design)" {
  payload=$(_mk 'comemory doctor')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: comemory stats is allowed (global by design)" {
  payload=$(_mk 'comemory stats')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

# An env prefix is NOT scope for comemory (no repo env var) — the bare
# `comemory search` underneath must still be denied for missing --repo.
@test "comemory-scope: env prefix before bare comemory search is denied" {
  payload=$(_mk 'FOO=x comemory search "x"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

# Regression: env-prefix strip regex stopped the value at the first space, so
# a quoted env value with whitespace (MY_VAR="foo bar") left the tail
# (bar" comemory save) unmatched by the ^comemory check — an unscoped raw call
# slipped through. A quoted value must be consumed whole, leaving `comemory
# save` as the segment, which is then denied for missing scope.
@test "comemory-scope: quoted env value with whitespace before bare comemory save is denied" {
  payload=$(_mk 'MY_VAR="foo bar" comemory save body --kind note')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

# Regression: lowercase env-var prefixes (foo=bar comemory …) are legal in bash.
# The strip regex only matched uppercase NAME chars, so the lowercase prefix was
# left in place and the segment no longer matched ^comemory — the unscoped raw
# call slipped through. Widening the NAME class keeps it denied.
@test "comemory-scope: lowercase env prefix before bare comemory save is denied" {
  payload=$(_mk 'foo=bar comemory save body --kind note')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: chained — bare comemory search after && is denied" {
  payload=$(_mk 'ls && comemory search "x"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "deny" ]
}

@test "comemory-scope: bare comemory with no subcommand is allowed (will fail at CLI)" {
  payload=$(_mk 'comemory')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: deny message mentions wrapper path" {
  payload=$(_mk 'comemory search "x"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("comemory.sh")'
}

# Tight assertion: the deny message must surface the STABLE published-path
# template the agent's Bash subshell can expand on either host, NOT an ephemeral
# plugin-root path. Regressing to a plugin-root path would re-trigger the
# "empty results = not-found" bug class this fix exists to close. Match the
# exact template (literal $ signs, single-quoted so they don't expand here).
@test "comemory-scope: deny message teaches the stable host-native template" {
  payload=$(_mk 'comemory search "x"')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  # The template appears verbatim (with literal $); fixed-string substring check.
  echo "$reason" | grep -qF '${TOOLU_CONFIG_DIR:-${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}}/comemory/comemory.sh'
  # Belt-and-braces: the old plugin-root path must NOT appear.
  ! echo "$reason" | grep -qF 'CLAUDE_PLUGIN_ROOT'
  ! echo "$reason" | grep -qF 'skills/agent-memory/scripts/comemory.sh'
}

@test "comemory-scope: semicolon inside quoted save arg does not falsely deny" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  payload=$(_mk 'comemory save "title; with semicolon" --kind note --repo foo')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: double-quoted &&  inside arg does not split" {
  command -v python3 >/dev/null 2>&1 || skip "python3 not installed"
  payload=$(_mk 'comemory save "a && b" --kind note --repo foo')
  run bash -c "tool_name=Bash input='$payload' bash '$HOOK'"
  [ "$status" -eq 0 ]
  [ "$(_decision "$output")" = "allow" ]
}

@test "comemory-scope: module ships in the plugin's pre-tools.d source dir" {
  # The module no longer lives in core's modules/ glob — it reaches the
  # dispatcher via the runtime registry (register.sh sync). This asserts the
  # source-of-truth location register.sh mirrors.
  [ -f "${BATS_TEST_DIRNAME}/../comemory-scope.sh" ]
}

@test "comemory-scope: exits 0 silently when TOOLU_LIB_DIR is unset (fail soft)" {
  payload=$(_mk 'comemory search "x"')
  run env -u TOOLU_LIB_DIR tool_name=Bash input="$payload" bash "$HOOK"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
