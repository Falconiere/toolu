#!/usr/bin/env bats
# Tests for the shared dispatcher (hooks/lib/dispatch.sh) under PreToolUse
# semantics, as used by hooks/pre-tools/mod.sh.
#
# Guarantees:
#   - permissionDecision:deny short-circuits subsequent modules.
#   - advisory additionalContext from multiple modules is merged into one
#     final output object (a single advisory does NOT preempt a later deny).
#   - a deny from any module wins even if alphabetically-earlier modules
#     produced advisory output first.
#   - module exit code 2 (Claude Code block convention) propagates: the
#     dispatcher returns 2 and forwards the module's stderr.
#   - any other non-zero module exit is logged and skipped; dispatch continues.

setup() {
  TMP=$(mktemp -d)
  MODULES_DIR="$TMP/modules"
  mkdir -p "$MODULES_DIR"

  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  # shellcheck source=../../lib/dispatch.sh
  . "$REPO_ROOT/hooks/lib/dispatch.sh"

  input='{"tool_name":"Bash","tool_input":{"command":"ls"}}'
  export input
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

write_module() {
  local name="$1"
  local body="$2"
  local path="$MODULES_DIR/${name}.sh"
  printf '%s\n' '#!/usr/bin/env bash' "$body" > "$path"
  chmod +x "$path"
}

@test "dispatcher: advisory from earlier module does NOT preempt a deny from later module" {
  # Alphabetically first: advisory.
  write_module "a_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"advisory-A\"}}"'
  # Alphabetically later: deny.
  write_module "z_deny"     'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"deny\",permissionDecisionReason:\"blocked-by-Z\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecisionReason | test("blocked-by-Z")'
}

@test "dispatcher: two advisory modules merge into ONE final output" {
  write_module "a_one" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"context-one\"}}"'
  write_module "b_two" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"context-two\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  # Exactly one JSON object on stdout.
  count=$(echo "$output" | jq -s 'length')
  [ "$count" = "1" ]
  echo "$output" | jq -e '.hookSpecificOutput.hookEventName == "PreToolUse"'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("context-one")'
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("context-two")'
}

@test "normalized dispatcher: a deny on any apply_patch path blocks the whole patch" {
  write_module "a_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"seen-path\"}}"'
  write_module "z_deny" 'p=$(jq -r ".tool_input.file_path" -); if [ "$p" = "hooks/lib/dispatch.sh" ]; then jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"deny\",permissionDecisionReason:\"protected-second-path\"}}"; fi'
  patch=$'*** Begin Patch\n*** Update File: src/safe.ts\n@@\n-a\n+b\n*** Update File: hooks/lib/dispatch.sh\n@@\n-a\n+b\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  echo "$output" | grep -q 'protected-second-path'
  ! echo "$output" | grep -q 'seen-path'
}

@test "normalized dispatcher: duplicate advisories across patch paths are emitted once" {
  write_module "a_same" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"same-advisory\"}}"'
  patch=$'*** Begin Patch\n*** Add File: a.ts\n+x\n*** Add File: b.ts\n+y\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.additionalContext' | grep -c '^same-advisory$')" = 1 ]
}

@test "normalized dispatcher: malformed apply_patch fails closed before modules run" {
  write_module "a_never" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"must-not-run\"}}"'
  patch=$'*** Begin Patch\n*** Update File: a.ts'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  ! echo "$output" | grep -q 'must-not-run'
}

@test "normalized dispatcher: missing jq fails open instead of reporting a malformed patch" {
  patch=$'*** Begin Patch\n*** Update File: a.ts\n@@\n-a\n+b\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  empty_path="$TMP/no-jq"
  mkdir -p "$empty_path"

  run env PATH="$empty_path" /bin/bash -c '
    . "$1"
    toolu_dispatch_modules() { printf "%s\n" modules-ran; }
    input="$2"
    tool_name=apply_patch
    export input tool_name
    toolu_dispatch_hook "$3" PreToolUse
  ' _ "$REPO_ROOT/hooks/lib/dispatch.sh" "$input" "$MODULES_DIR"

  [ "$status" -eq 0 ]
  [ "$output" = modules-ran ]
}

@test "pre-tools entrypoint blocks a protected path hidden later in a Codex patch" {
  patch=$'*** Begin Patch\n*** Update File: README.md\n@@\n-a\n+b\n*** Update File: plugins/toolu/hooks/lib/dispatch.sh\n@@\n-a\n+b\n*** End Patch'
  payload=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex" TOOLU_PROJECT_DIR="$REPO_ROOT" \
    MY_CLAUDE_QUALITY=off bash "$REPO_ROOT/hooks/pre-tools/mod.sh" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  echo "$output" | grep -q 'plugins/toolu/hooks/lib/dispatch.sh'
}

@test "pre-tools entrypoint lets an apply_patch through while the quality gate is failing" {
  project="$TMP/project"
  mkdir -p "$project/.codex/tmp"
  git -C "$project" init -q
  git -C "$project" -c user.email=t@t -c user.name=t commit --allow-empty -qm init
  printf '%s\n' '{"status":"failing","reason":"tests failed","violations":"fix tests"}' \
    > "$project/.codex/tmp/quality-gate-status.json"
  # A failing gate stops SHIPPING, not working: edits stay open so the
  # violation can be fixed. Committing is what it denies — asserted below and
  # in quality-gate.bats.
  patch=$'*** Begin Patch\n*** Update File: src/fix.ts\n@@\n-a\n+b\n*** Update File: README.md\n@@\n-a\n+b\n*** End Patch'
  payload=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run bash -c 'cd "$1" && env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$2" TOOLU_PROJECT_DIR="$1" bash "$3" <<<"$4"' \
    _ "$project" "$TMP/codex" "$REPO_ROOT/hooks/pre-tools/mod.sh" "$payload"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision // "none"')" = "none" ]

  commit_payload=$(jq -cn '{tool_name:"Bash",tool_input:{command:"git commit -m \"feat: x\""}}')
  run bash -c 'cd "$1" && env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$2" TOOLU_PROJECT_DIR="$1" bash "$3" <<<"$4"' \
    _ "$project" "$TMP/codex" "$REPO_ROOT/hooks/pre-tools/mod.sh" "$commit_payload"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  echo "$output" | grep -q 'quality gate failing'
}

@test "dispatcher: deny short-circuits later modules (no trailing advisory after deny)" {
  write_module "a_deny"     'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"deny\",permissionDecisionReason:\"early-deny\"}}"'
  write_module "z_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"should-not-appear\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  count=$(echo "$output" | jq -s 'length')
  [ "$count" = "1" ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"'
  ! echo "$output" | grep -q "should-not-appear"
}

@test "dispatcher: silent modules produce no output" {
  write_module "a_silent" 'exit 0'
  write_module "b_silent" 'exit 0'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "dispatcher: modules receive hook input on stdin" {
  write_module "a_reader" 'tool=$(jq -r ".tool_name" -); jq -n --arg t "$tool" "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:(\"saw-\" + \$t)}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext == "saw-Bash"'
}

@test "dispatcher: module exit 2 propagates as block (status 2, stderr forwarded, later modules skipped)" {
  write_module "a_block"    'echo "hard-block-reason" >&2; exit 2'
  write_module "z_advisory" 'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"should-not-appear\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 2 ]
  # bats merges stderr into $output.
  echo "$output" | grep -q "hard-block-reason"
  ! echo "$output" | grep -q "should-not-appear"
}

@test "dispatcher: module failing with other non-zero exit is skipped, dispatch continues" {
  write_module "a_broken" 'echo "{\"hookSpecificOutput\":{\"additionalContext\":\"partial-garbage\"}}"; exit 1'
  write_module "b_good"   'jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"good-context\"}}"'

  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  # Failure is visible (warning names the module) but stdout from the failed
  # module is discarded.
  echo "$output" | grep -q "a_broken.sh"
  ! echo "$output" | grep -q "partial-garbage"
  echo "$output" | grep -q "good-context"
}

@test "dispatcher: empty modules dir is a no-op" {
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "dispatch runs a registry module from an active plugin" {
  builtin_dir=$(mktemp -d); reg_dir=$(mktemp -d)
  cat > "$reg_dir/comemory@toolu__probe.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"from-registry"}}'
EOF
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/detect.sh"; . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    toolu_plugin_active() { return 0; }       # force active
    input="{}"; tool_name="Read"; export input tool_name
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("from-registry")' >/dev/null
  rm -rf "$builtin_dir" "$reg_dir"
}

@test "dispatch SKIPS a registry module whose plugin is inactive" {
  builtin_dir=$(mktemp -d); reg_dir=$(mktemp -d)
  cat > "$reg_dir/ghost@nowhere__probe.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"should-not-appear"}}'
EOF
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/detect.sh"; . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    toolu_plugin_active() { return 1; }        # force inactive
    input="{}"; tool_name="Read"; export input tool_name
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  rm -rf "$builtin_dir" "$reg_dir"
}

@test "dispatch SKIPS an un-namespaced file in a registry dir (never runs ungated)" {
  builtin_dir="$TMP/builtin"; reg_dir="$TMP/reg"
  mkdir -p "$builtin_dir" "$reg_dir"
  cat > "$reg_dir/foo.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"ungated-should-not-appear"}}'
EOF
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/detect.sh"; . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    toolu_plugin_active() { return 0; }
    input="{}"; export input
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "ungated-should-not-appear"
  # The skip is visible on stderr (merged into $output by bats).
  echo "$output" | grep -q "foo.sh"
}

@test "dispatch SKIPS a registry file with an empty plugin spec (__name.sh)" {
  builtin_dir="$TMP/builtin"; reg_dir="$TMP/reg"
  mkdir -p "$builtin_dir" "$reg_dir"
  cat > "$reg_dir/__sneaky.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"empty-spec-should-not-appear"}}'
EOF
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/detect.sh"; . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    input="{}"; export input
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "empty-spec-should-not-appear"
}

@test "dispatch SKIPS registry modules when toolu_plugin_active is not sourced (fail closed)" {
  builtin_dir="$TMP/builtin"; reg_dir="$TMP/reg"
  mkdir -p "$builtin_dir" "$reg_dir"
  cat > "$builtin_dir/00-builtin.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"builtin-still-runs"}}'
EOF
  cat > "$reg_dir/ghost@nowhere__probe.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"registry-should-not-appear"}}'
EOF
  # dispatch.sh sourced WITHOUT detect.sh: helper undeclared.
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    input="{}"; export input
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "builtin-still-runs"
  ! echo "$output" | grep -q "registry-should-not-appear"
}

@test "dispatch resolves each plugin spec at most once per dispatch (memoized)" {
  builtin_dir="$TMP/builtin"; reg_dir="$TMP/reg"; counter="$TMP/lookup-count"
  mkdir -p "$builtin_dir" "$reg_dir"
  local n
  for n in one two three; do
    printf '%s\n' '#!/usr/bin/env bash' 'exit 0' > "$reg_dir/comemory@toolu__$n.sh"
  done
  printf '%s\n' '#!/usr/bin/env bash' 'exit 0' > "$reg_dir/other@toolu__solo.sh"
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    toolu_plugin_active() { echo "$1" >> "'"$counter"'"; return 0; }
    input="{}"; export input
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  # 4 registry modules, 2 distinct specs -> exactly 2 lookups.
  [ "$(wc -l < "$counter" | tr -d ' ')" = "2" ]
  [ "$(sort -u "$counter" | wc -l | tr -d ' ')" = "2" ]
}

@test "dispatch gates ALL registry files when modules_dir is empty (no builtin bypass)" {
  reg_dir="$TMP/reg"
  mkdir -p "$reg_dir"
  cat > "$reg_dir/foo.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"bypass-should-not-appear"}}'
EOF
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/detect.sh"; . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    toolu_plugin_active() { return 0; }
    input="{}"; export input
    toolu_dispatch_modules "" "PreToolUse" "'"$reg_dir"'"
  '
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "bypass-should-not-appear"
}

@test "dispatch still works with no registry dir argument (back-compat)" {
  builtin_dir=$(mktemp -d)
  cat > "$builtin_dir/00-a.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"builtin"}}'
EOF
  run bash -c '
    . "'"$REPO_ROOT"'/hooks/lib/dispatch.sh"
    input="{}"; tool_name="Read"; export input tool_name
    toolu_dispatch_modules "'"$builtin_dir"'" "PreToolUse"
  '
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("builtin")' >/dev/null
  rm -rf "$builtin_dir"
}

@test "pre-tools entrypoint exports TOOLU_LIB_DIR to modules" {
  # The probe module goes into a COPY of the hooks tree, never the live one.
  # Writing it into the real modules dir made this test mutate the tree every
  # other test runs against: a concurrent run could execute a half-written
  # probe, or delete it out from under this one. The entrypoint under test is
  # still the real script — mod.sh resolves its lib dir from its own location,
  # so a faithful copy exercises exactly the same code path.
  cp -R "$REPO_ROOT/hooks" "$TMP/hooks"
  local probe="$TMP/hooks/pre-tools/modules/zzz-libdir-probe.sh"
  cat > "$probe" <<'EOF'
#!/usr/bin/env bash
jq -n --arg v "${TOOLU_LIB_DIR:-UNSET}" \
  '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:("LIBDIRPROBE="+$v)}}'
EOF
  chmod +x "$probe"

  # Run the entrypoint with the env var UNSET: only mod.sh's own export can
  # make the probe see a value.
  run env -u TOOLU_LIB_DIR bash "$TMP/hooks/pre-tools/mod.sh" <<<'{"tool_name":"Read"}'

  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("LIBDIRPROBE=/") and (contains("LIBDIRPROBE=UNSET")|not)' >/dev/null
  # The lib dir the modules saw must be the copy's own, not the live tree's.
  echo "$output" | jq -e --arg tmp "$TMP" '.hookSpecificOutput.additionalContext | contains("LIBDIRPROBE=" + $tmp)' >/dev/null

  # And the live tree is untouched.
  [ ! -e "$REPO_ROOT/hooks/pre-tools/modules/zzz-libdir-probe.sh" ]
}

@test "pre-tools entrypoint executes an active-plugin registry module" {
  cfg="$TMP/e2e-cfg"
  regdir="$cfg/toolu/pre-tools.d"; mkdir -p "$regdir"
  cat > "$regdir/comemory@toolu__probe.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"e2e-registry"}}'
EOF
  # Make comemory@toolu read as installed. With CLAUDE_CONFIG_DIR set,
  # detect_plugin_installed reads <config-dir>/plugins/installed_plugins.json
  # — NOT <HOME>/.claude/plugins/. Writing the wrong path passes via fail-open
  # (manifest missing = indeterminate) and silently stops testing the gate.
  mkdir -p "$cfg/plugins"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$cfg/plugins/installed_plugins.json"
  # macOS BSD `env` requires option flags (-u) before VAR=val operands.
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$cfg" HOME="$cfg" \
    bash "$REPO_ROOT/hooks/pre-tools/mod.sh" <<<'{"tool_name":"Read"}'
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | contains("e2e-registry")' >/dev/null
}

@test "pre-tools entrypoint SKIPS a registry module whose plugin is definitively absent" {
  cfg="$TMP/e2e-cfg-absent"
  regdir="$cfg/toolu/pre-tools.d"; mkdir -p "$regdir"
  cat > "$regdir/comemory@toolu__probe.sh" <<'EOF'
#!/usr/bin/env bash
jq -n '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:"should-not-appear"}}'
EOF
  # Manifest EXISTS and the spec is absent: definitively not installed
  # (rules out the fail-open path a missing manifest would take).
  mkdir -p "$cfg/plugins"
  printf '%s' '{"plugins":{}}' > "$cfg/plugins/installed_plugins.json"
  run env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="$cfg" HOME="$cfg" \
    bash "$REPO_ROOT/hooks/pre-tools/mod.sh" <<<'{"tool_name":"Read"}'
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "should-not-appear"
}

# ── ask handling (AC-21, AC-22) ─────────────────────────────────────────────
#
# `ask` is held rather than emitted on sight: a later module must still be able
# to deny, and an advisory raised after the ask must ride along on its reason.

@test "ask reaches the caller as an ask decision" {
  write_module "10-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"push without a review?\"}}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" = "push without a review?" ]
}

@test "a later deny outranks an earlier ask" {
  write_module "10-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"prompt me\"}}"'
  write_module "20-deny" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"protected file\"}}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" = "protected file" ]
}

@test "an earlier deny still short-circuits before a later ask" {
  write_module "10-deny" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"blocked first\"}}"'
  write_module "20-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"prompt me\"}}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "deny" ]
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" = "blocked first" ]
}

@test "the first ask wins when two modules ask" {
  write_module "10-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"first\"}}"'
  write_module "20-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"second\"}}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')" = "first" ]
}

@test "an advisory raised after an ask is appended to the ask reason" {
  write_module "10-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"push without a review?\"}}"'
  write_module "20-advice" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"additionalContext\":\"docs look stale\"}}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecision')" = "ask" ]
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"push without a review?"* ]]
  [[ "$reason" == *"docs look stale"* ]]
}

@test "an advisory raised before an ask is also appended" {
  write_module "10-advice" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"additionalContext\":\"gate is red\"}}"'
  write_module "20-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"push anyway?\"}}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"push anyway?"* ]]
  [[ "$reason" == *"gate is red"* ]]
}

@test "a systemMessage from another module survives an ask" {
  write_module "10-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"prompt\"}}"'
  write_module "20-msg" 'echo "{\"systemMessage\":\"heads up\"}"'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$(echo "$output" | jq -r '.systemMessage')" = "heads up" ]
}

@test "a module exiting 2 after an ask still hard-blocks" {
  write_module "10-ask" 'echo "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"ask\",\"permissionDecisionReason\":\"prompt\"}}"'
  write_module "20-exit2" 'echo "hard block reason" >&2; exit 2'
  run toolu_dispatch_modules "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 2 ]
}

@test "normalized dispatcher: an ask on one apply_patch path prompts for the whole patch" {
  write_module "a_ask" 'p=$(jq -r ".tool_input.file_path" -); if [ "$p" = "src/risky.ts" ]; then jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"ask\",permissionDecisionReason:\"risky-path\"}}"; fi'
  patch=$'*** Begin Patch\n*** Update File: src/safe.ts\n@@\n-a\n+b\n*** Update File: src/risky.ts\n@@\n-a\n+b\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PreToolUse"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "ask"' >/dev/null
  echo "$output" | grep -q 'risky-path'
}

@test "normalized dispatcher: a deny on a later path outranks an ask on an earlier one" {
  write_module "a_mixed" 'p=$(jq -r ".tool_input.file_path" -); case "$p" in
    src/first.ts) jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"ask\",permissionDecisionReason:\"ask-first\"}}" ;;
    hooks/lib/dispatch.sh) jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"deny\",permissionDecisionReason:\"deny-second\"}}" ;;
  esac'
  patch=$'*** Begin Patch\n*** Update File: src/first.ts\n@@\n-a\n+b\n*** Update File: hooks/lib/dispatch.sh\n@@\n-a\n+b\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PreToolUse"
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  echo "$output" | grep -q 'deny-second'
}

@test "normalized dispatcher: an advisory from another path rides on the ask reason" {
  write_module "a_mixed" 'p=$(jq -r ".tool_input.file_path" -); case "$p" in
    src/first.ts) jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",permissionDecision:\"ask\",permissionDecisionReason:\"ask-first\"}}" ;;
    src/second.ts) jq -n "{hookSpecificOutput:{hookEventName:\"PreToolUse\",additionalContext:\"advice-second\"}}" ;;
  esac'
  patch=$'*** Begin Patch\n*** Update File: src/first.ts\n@@\n-a\n+b\n*** Update File: src/second.ts\n@@\n-a\n+b\n*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  tool_name=apply_patch
  export input tool_name
  run toolu_dispatch_hook "$MODULES_DIR" "PreToolUse"
  reason=$(echo "$output" | jq -r '.hookSpecificOutput.permissionDecisionReason')
  [[ "$reason" == *"ask-first"* ]]
  [[ "$reason" == *"advice-second"* ]]
}
