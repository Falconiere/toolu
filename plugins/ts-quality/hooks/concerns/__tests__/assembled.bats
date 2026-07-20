#!/usr/bin/env bats
# Equivalence harness for the ts-quality plugin's ASSEMBLED registry module.
#
# register.sh concatenates concerns/[0-9][0-9]-*.sh into one runtime module. That
# module must produce the SAME quality-gate-status.json as the original monolith
# (plugins/lang-quality/hooks/post-tools.d/ts-quality.sh). These tests drive BOTH
# on real git-init'd TS projects and diff the resulting gate JSON, so a fragment
# reorder / clobber / dropped-suppression regression fails here.

# Core lib lives in the sibling toolu plugin; the dispatcher provides this
# env var in production, the tests provide it here.
TOOLU_LIB_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../../toolu/hooks/lib" && pwd)"
export TOOLU_LIB_DIR

# The pre-split monolith — the behavioral oracle.
# Built from the existing plugins/ dir so it resolves even after lang-quality is
# deleted (the file then simply does not exist → the oracle tests skip).
MONOLITH="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)/lang-quality/hooks/post-tools.d/ts-quality.sh"

setup() {
  TMP=$(mktemp -d)

  # Assemble the registry module exactly as production does.
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)/register.sh"
  bash "$REGISTER" </dev/null
  ASSEMBLED="$CLAUDE_CONFIG_DIR/toolu/post-tools.d/ts-quality@toolu__ts-quality.sh"

  PROJ="$TMP/proj"
  mkdir -p "$PROJ"
  cd "$PROJ"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Minimal TS project (tsconfig + bun lockfile) so detect_ts/detect_node_pm pass.
# The tsconfig declares a `@/*` path alias so the `../ import` rule (now gated on
# the alias existing) fires — the fixtures below that assert on it rely on this.
_ts_project() {
  echo '{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}' > "$PROJ/tsconfig.json"
  echo '{"name":"x"}' > "$PROJ/package.json"
  touch "$PROJ/bun.lock"
  mkdir -p "$PROJ/src"
  ( cd "$PROJ" && git add -A && git -c user.email=t@t -c user.name=t commit -q -m setup )
}

# Same minimal project but WITHOUT a `@/` path alias — the plain-NodeNext case
# where a `../` import has no alias to move to.
_ts_project_no_alias() {
  echo '{}' > "$PROJ/tsconfig.json"
  echo '{"name":"x"}' > "$PROJ/package.json"
  touch "$PROJ/bun.lock"
  mkdir -p "$PROJ/src"
  ( cd "$PROJ" && git add -A && git -c user.email=t@t -c user.name=t commit -q -m setup )
}

# Run a given hook script on a fixture file; print the resulting gate JSON.
# Failing and passing branches both target $PROJECT_ROOT/.claude/tmp; print
# whichever exists.
_run_gate() {
  local hook="$1" file="$2"
  rm -rf "$PROJ/.claude/tmp"
  local payload='{"tool_input":{"file_path":"'"$file"'"}}'
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$hook" >/dev/null 2>&1
  local gate="$PROJ/.claude/tmp/quality-gate-status.json"
  if [ -f "$gate" ]; then cat "$gate"; fi
}

# Gate JSON with every updatedAt stripped (the only legitimately run-varying
# field). Everything else must match between monolith and assembled.
_gate_canon() {
  _run_gate "$1" "$2" | jq -S 'del(.updatedAt) | (.entries // {}) |= with_entries(.value |= del(.updatedAt))'
}

@test "assembled module exists after register.sh" {
  [ -f "$ASSEMBLED" ]
}

# --- Deliverable A.2: oracle diff on a 3-violation fixture ---
# Catches fragment reorder/clobber: a .ts file with an `as` cast AND a
# console.log AND a `../` import. The gate must list all three in the SAME order
# from both monolith and assembled.
@test "ts assembled == monolith on a 3-violation fixture (oracle diff)" {
  [ -f "$MONOLITH" ] || skip "monolith removed post-split; equivalence proven pre-delete"
  _ts_project
  cat > "$PROJ/src/bad.ts" <<'EOF'
import { thing } from "../other";
export const x = thing as Bar;
export function go() {
  console.log("debug");
}
EOF
  mono=$(_gate_canon "$MONOLITH" "$PROJ/src/bad.ts")
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.ts")

  # Guard the oracle: all three violations present and failing.
  echo "$mono" | jq -e '.status == "failing"'
  echo "$mono" | jq -e '.violations | contains("Forbidden ../ import")'
  echo "$mono" | jq -e ".violations | contains(\"Forbidden 'as' type assertion\")"
  echo "$mono" | jq -e '.violations | contains("Forbidden console.log")'
  # Ordered: imports (10) before type-as (15) before console (60). Compare byte
  # offsets of each marker within the single violations string (newline-encoding
  # agnostic).
  v=$(echo "$mono" | jq -r '.violations')
  before_import="${v%%Forbidden ../ import*}"
  before_as="${v%%Forbidden \'as\' type assertion*}"
  before_console="${v%%Forbidden console.log*}"
  [ "${#before_import}" -lt "${#before_as}" ]
  [ "${#before_as}" -lt "${#before_console}" ]

  # The crux: identical gate JSON.
  [ "$mono" = "$asm" ]
}

# --- Deliverable A.3: no clobber — fix ONE of three violations ---
@test "ts assembled keeps gate failing after fixing one of three violations" {
  _ts_project
  cat > "$PROJ/src/bad.ts" <<'EOF'
import { thing } from "../other";
export const x = thing as Bar;
export function go() {
  console.log("debug");
}
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.ts")
  echo "$asm" | jq -e '.status == "failing"'

  # Fix ONLY the console.log — the `../` import and `as` cast remain.
  cat > "$PROJ/src/bad.ts" <<'EOF'
import { thing } from "../other";
export const x = thing as Bar;
export function go() {
  console.info("debug");
}
EOF
  still=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.ts")
  echo "$still" | jq -e '.status == "failing"'
  echo "$still" | jq -e '.violations | contains("Forbidden ../ import")'
  echo "$still" | jq -e ".violations | contains(\"Forbidden 'as' type assertion\")"
  echo "$still" | jq -e '.violations | (contains("Forbidden console.log") | not)'
}

# --- ../ import rule is gated on a `@/` alias actually existing ---
# Regression: the rule fired unconditionally and told the author to "use @/
# alias" even in a repo with no such alias — an unactionable false positive that
# blocked every edit to the file. With no alias configured, the `../` import must
# NOT be flagged; an unrelated `as` cast in the same file still must, so the gate
# is scoped to the import rule alone, not silenced wholesale.
@test "ts assembled: ../ import NOT flagged when the project defines no @/ alias" {
  _ts_project_no_alias
  cat > "$PROJ/src/bad.ts" <<'EOF'
import { thing } from "../other";
export const x = thing as Bar;
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.ts")
  echo "$asm" | jq -e '.status == "failing"'                                   # the `as` cast still fails the gate
  echo "$asm" | jq -e ".violations | contains(\"Forbidden 'as' type assertion\")"
  echo "$asm" | jq -e '.violations | (contains("Forbidden ../ import") | not)' # ...but the import is not flagged
}

# Inverse: with a `@/` alias configured, the same `../` import IS flagged.
@test "ts assembled: ../ import IS flagged when the project defines a @/ alias" {
  _ts_project   # tsconfig here declares paths: { "@/*": ["./src/*"] }
  cat > "$PROJ/src/bad.ts" <<'EOF'
import { thing } from "../other";
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.ts")
  echo "$asm" | jq -e '.status == "failing"'
  echo "$asm" | jq -e '.violations | contains("Forbidden ../ import")'
}

# --- Deliverable A.4: suppression preserved ---
# @ts-expect-error is forbidden in src/ but ALLOWED in a test file. The
# assembled module must keep that suppression carve-out and match the monolith.
@test "ts assembled: @ts-expect-error allowed in test files (suppression preserved)" {
  _ts_project
  mkdir -p "$PROJ/src/__tests__"
  cat > "$PROJ/src/__tests__/code.test.ts" <<'EOF'
// @ts-expect-error asserting a type error is the point of this test
const z: string = 123;
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/__tests__/code.test.ts")
  # The carve-out holds: no suppression violation, so no failing gate is written
  # (gate_clear_file finds no prior entry → no gate file). The assembled module
  # must keep that carve-out and match the monolith (here: both produce nothing).
  [ -z "$asm" ] || echo "$asm" | jq -e '((.violations // "") | contains("Forbidden suppression comment")) | not'
  [ -z "$asm" ] || echo "$asm" | jq -e '.status != "failing"'
  if [ -f "$MONOLITH" ]; then
    [ "$(_gate_canon "$MONOLITH" "$PROJ/src/__tests__/code.test.ts")" = "$asm" ]
  fi
}

# Inverse guard for the same suppression rule: in src/ it IS flagged, identically.
@test "ts assembled: @ts-expect-error in src/ is flagged == monolith" {
  _ts_project
  cat > "$PROJ/src/bad.ts" <<'EOF'
// @ts-expect-error legacy boundary
export const b = thing();
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/bad.ts")
  echo "$asm" | jq -e '.violations | contains("Forbidden suppression comment")'
  if [ -f "$MONOLITH" ]; then
    [ "$(_gate_canon "$MONOLITH" "$PROJ/src/bad.ts")" = "$asm" ]
  fi
}

# --- Deliverable A.5: clean file clears the gate ---
@test "ts assembled: clean file writes no failure (gate passing) and == monolith" {
  _ts_project
  cat > "$PROJ/src/good.ts" <<'EOF'
/** Adds two numbers. */
export function add(a: number, b: number): number {
  return a + b;
}
EOF
  asm=$(_gate_canon "$ASSEMBLED" "$PROJ/src/good.ts")
  # A clean file on a project with no prior gate writes NO failure (gate_clear_file
  # finds no entry → no gate file). The assembled module must do the same and
  # match the monolith (here: both produce nothing).
  [ -z "$asm" ] || echo "$asm" | jq -e '.status != "failing"'
  if [ -f "$MONOLITH" ]; then
    [ "$(_gate_canon "$MONOLITH" "$PROJ/src/good.ts")" = "$asm" ]
  fi
}

# Stronger criterion-5: the assembled module's CLEAR path actively flips a prior
# failure (recorded by the same hook+file) to passing — not just "writes nothing".
@test "ts assembled: re-editing a failing file clean flips the gate to passing" {
  _ts_project
  GATE="$PROJ/.claude/tmp/quality-gate-status.json"
  payload='{"tool_input":{"file_path":"'"$PROJ"'/src/x.ts"}}'

  # Phase 1: a violating file (throw of a numeric literal) records a failing gate.
  printf 'export function b(){ throw 42; }\n' > "$PROJ/src/x.ts"
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$ASSEMBLED" >/dev/null 2>&1
  jq -e '.status == "failing"' "$GATE"
  jq -e '.source == "ts-quality-hook"' "$GATE"

  # Phase 2: same file goes clean → the assembled module clears it → passing.
  printf 'export function b(){ throw new Error("boom"); }\n' > "$PROJ/src/x.ts"
  TOOLU_LIB_DIR="$TOOLU_LIB_DIR" tool_name=Edit input="$payload" \
    PROJECT_ROOT="$PROJ" bash "$ASSEMBLED" >/dev/null 2>&1
  jq -e '.status == "passing"' "$GATE"
  jq -e '.source == "ts-quality-hook"' "$GATE"
}
