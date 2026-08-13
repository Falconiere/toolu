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
  # Always clean the real-modules probe, even if an assertion failed mid-test.
  [ -n "${REPO_ROOT:-}" ] && rm -f "$REPO_ROOT/hooks/pre-tools/modules/zzz-libdir-probe.sh"
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

@test "pre-tools entrypoint blocks a protected path hidden later in a Codex patch" {
  patch=$'*** Begin Patch\n*** Update File: README.md\n@@\n-a\n+b\n*** Update File: plugins/toolu/hooks/lib/dispatch.sh\n@@\n-a\n+b\n*** End Patch'
  payload=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex" TOOLU_PROJECT_DIR="$REPO_ROOT" \
    MY_CLAUDE_QUALITY=off bash "$REPO_ROOT/hooks/pre-tools/mod.sh" <<<"$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  echo "$output" | grep -q 'plugins/toolu/hooks/lib/dispatch.sh'
}

@test "pre-tools entrypoint applies an active failure gate to every Codex patch path" {
  project="$TMP/project"
  mkdir -p "$project/.codex/tmp"
  git -C "$project" init -q
  git -C "$project" -c user.email=t@t -c user.name=t commit --allow-empty -qm init
  printf '%s\n' '{"status":"failing","reason":"tests failed","violations":"fix tests"}' \
    > "$project/.codex/tmp/quality-gate-status.json"
  patch=$'*** Begin Patch\n*** Update File: src/fix.ts\n@@\n-a\n+b\n*** Update File: README.md\n@@\n-a\n+b\n*** End Patch'
  payload=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run bash -c 'cd "$1" && env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$2" TOOLU_PROJECT_DIR="$1" bash "$3" <<<"$4"' \
    _ "$project" "$TMP/codex" "$REPO_ROOT/hooks/pre-tools/mod.sh" "$payload"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.permissionDecision == "deny"' >/dev/null
  echo "$output" | grep -q 'Quality gate failing'
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
  # Drop a probe module into the REAL modules dir with a last-running name so
  # it is dispatched by the actual mod.sh entrypoint. It echoes back whatever
  # TOOLU_LIB_DIR it inherited from the entrypoint's exported environment.
  local probe="$REPO_ROOT/hooks/pre-tools/modules/zzz-libdir-probe.sh"
  # Belt #1: clear any stale probe left by a prior hard-killed run before writing.
  rm -f "$probe"
  # Belt #2: the probe self-deletes after emitting JSON, so a single execution
  # cleans itself even if bats is killed before teardown.
  cat > "$probe" <<'EOF'
#!/usr/bin/env bash
jq -n --arg v "${TOOLU_LIB_DIR:-UNSET}" \
  '{hookSpecificOutput:{hookEventName:"PreToolUse",additionalContext:("LIBDIRPROBE="+$v)}}'
rm -f "${BASH_SOURCE[0]}"
EOF
  chmod +x "$probe"

  # Run the REAL entrypoint with the env var UNSET: only mod.sh's own export
  # can make the probe see a value.
  run env -u TOOLU_LIB_DIR bash "$REPO_ROOT/hooks/pre-tools/mod.sh" <<<'{"tool_name":"Read"}'

  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.hookSpecificOutput.additionalContext | test("LIBDIRPROBE=/") and (contains("LIBDIRPROBE=UNSET")|not)' >/dev/null
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
