#!/usr/bin/env bats
# register.sh ASSEMBLES concerns/[0-9][0-9]-*.sh into ONE registry module
# <spec>__rust-quality.sh under $CLAUDE_CONFIG_DIR/toolu/post-tools.d/,
# prunes its own stale entries + tmp residue, and never touches other plugins.

SPEC="rust-quality@toolu"
MODULE="${SPEC}__rust-quality.sh"

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  HOOKS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"
  REGISTER="$HOOKS_DIR/register.sh"
  CONCERNS_DIR="$HOOKS_DIR/concerns"
  LIB_DIR="$HOOKS_DIR/lib"
  REG_DIR="$CLAUDE_CONFIG_DIR/toolu/post-tools.d"
}
teardown() { rm -rf "$TMP"; }

# Reproduce the assembly independently of register.sh: in-order concat of
# (1) the preamble 00-*.sh, (2) the shared lib/*.sh, (3) the remaining
# numeric-prefixed concern fragments — each followed by a newline.
_expected_module() {
  local out="$1"; : > "$out"
  local f seen_preamble=""
  for f in "$CONCERNS_DIR"/00-*.sh "$LIB_DIR"/*.sh "$CONCERNS_DIR"/[0-9][0-9]-*.sh; do
    [ -f "$f" ] || continue
    case "$f" in "$CONCERNS_DIR"/00-*.sh) [ -z "$seen_preamble" ] || continue; seen_preamble=1 ;; esac
    cat "$f" >> "$out"
    printf '\n' >> "$out"
  done
}

# --- B.1: exactly ONE assembled file, not one-per-fragment ---
@test "register: produces exactly ONE module file for our spec prefix" {
  run bash "$REGISTER" </dev/null
  [ "$status" -eq 0 ]
  [ -f "$REG_DIR/$MODULE" ]
  # Count files bearing our prefix — must be exactly 1 (the assembled module).
  count=$(find "$REG_DIR" -maxdepth 1 -name "${SPEC}__*.sh" | wc -l | tr -d ' ')
  [ "$count" -eq 1 ]
}

# --- B.2: assembled bytes == in-order concat of fragments ---
@test "register: module bytes equal the ordered concat of concerns/[0-9][0-9]-*.sh" {
  bash "$REGISTER" </dev/null
  _expected_module "$TMP/expected.sh"
  cmp "$TMP/expected.sh" "$REG_DIR/$MODULE"
}

# --- B.3: prune our own stale entry, keep foreign ---
@test "register: prunes its own stale <spec>__old-concern.sh, keeps other plugins'" {
  mkdir -p "$REG_DIR"
  echo stale > "$REG_DIR/${SPEC}__old-concern.sh"
  echo keep  > "$REG_DIR/other@market__keep.sh"
  run bash "$REGISTER" </dev/null
  [ "$status" -eq 0 ]
  [ ! -f "$REG_DIR/${SPEC}__old-concern.sh" ]
  [ -f "$REG_DIR/other@market__keep.sh" ]
  # Only the assembled module remains for our prefix.
  count=$(find "$REG_DIR" -maxdepth 1 -name "${SPEC}__*.sh" | wc -l | tr -d ' ')
  [ "$count" -eq 1 ]
  [ -f "$REG_DIR/$MODULE" ]
}

# --- B.4: idempotency — same bytes, unchanged mtime on a second run ---
@test "register: idempotent — second run leaves the module byte- and mtime-stable, silent" {
  bash "$REGISTER" </dev/null
  before_sum=$(cksum < "$REG_DIR/$MODULE")
  before_mtime=$({ stat -c %Y "$REG_DIR/$MODULE" 2>/dev/null || stat -f %m "$REG_DIR/$MODULE"; })
  # Ensure a real clock tick could be observed if the file were rewritten.
  sleep 1
  run bash "$REGISTER" </dev/null
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  after_sum=$(cksum < "$REG_DIR/$MODULE")
  after_mtime=$({ stat -c %Y "$REG_DIR/$MODULE" 2>/dev/null || stat -f %m "$REG_DIR/$MODULE"; })
  [ "$before_sum" = "$after_sum" ]
  # mv only happens when bytes differ, so mtime must be untouched.
  [ "$before_mtime" = "$after_mtime" ]
}

# --- B.5: aged tmp residue cleaned, fresh + foreign kept ---
@test "register: clears its own AGED tmp residue, keeps fresh + foreign tmp" {
  mkdir -p "$REG_DIR"
  echo x > "$REG_DIR/${SPEC}__rust-quality.sh.tmp.111"
  touch -t 202601010000 "$REG_DIR/${SPEC}__rust-quality.sh.tmp.111"
  echo x > "$REG_DIR/${SPEC}__rust-quality.sh.tmp.222"   # fresh — kept
  echo x > "$REG_DIR/other@market__keep.sh.tmp.9"; touch -t 202601010000 "$REG_DIR/other@market__keep.sh.tmp.9"
  run bash "$REGISTER" </dev/null
  [ "$status" -eq 0 ]
  [ ! -f "$REG_DIR/${SPEC}__rust-quality.sh.tmp.111" ]
  [ -f "$REG_DIR/${SPEC}__rust-quality.sh.tmp.222" ]
  [ -f "$REG_DIR/other@market__keep.sh.tmp.9" ]
}

# End-to-end: register -> registry -> core PostToolUse dispatcher. Uses the
# Rust inline-#[allow] suppression rule (no cargo/ast-grep needed) for a
# deterministic signal. cwd must be the project so mod.sh's PROJECT_ROOT
# (git rev-parse) resolves there.
_rust_dispatch_project() {
  proj="$TMP/rs"; mkdir -p "$proj/src"
  ( cd "$proj" && git init -q \
    && printf '[package]\nname = "x"\nversion = "0.1.0"\n' > Cargo.toml \
    && printf '#[allow(dead_code)]\nfn helper() {}\n' > src/bad.rs \
    && git add -A && git -c user.email=t@t -c user.name=t commit -q -m s )
  CORE_MOD="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../toolu/hooks/post-tools" && pwd)/mod.sh"
}

@test "register e2e: assembled module fires through the core dispatcher when installed" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_dispatch_project
  bash "$REGISTER" </dev/null
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{"rust-quality@toolu":{}}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  run bash -c 'cd "'"$proj"'" && env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="'"$CLAUDE_CONFIG_DIR"'" HOME="'"$TMP"'" bash "'"$CORE_MOD"'" <<<'"'"'{"tool_name":"Edit","tool_input":{"file_path":"'"$proj"'/src/bad.rs"},"tool_response":{}}'"'"''
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "Forbidden lint suppression"
}

@test "register e2e: assembled module is gated off when the plugin is absent" {
  command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
  _rust_dispatch_project
  bash "$REGISTER" </dev/null
  mkdir -p "$CLAUDE_CONFIG_DIR/plugins"
  printf '%s' '{"plugins":{}}' > "$CLAUDE_CONFIG_DIR/plugins/installed_plugins.json"
  run bash -c 'cd "'"$proj"'" && env -u CLAUDE_PLUGINS_REGISTRY CLAUDE_CONFIG_DIR="'"$CLAUDE_CONFIG_DIR"'" HOME="'"$TMP"'" bash "'"$CORE_MOD"'" <<<'"'"'{"tool_name":"Edit","tool_input":{"file_path":"'"$proj"'/src/bad.rs"},"tool_response":{}}'"'"''
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "Forbidden lint suppression"
}
