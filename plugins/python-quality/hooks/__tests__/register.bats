#!/usr/bin/env bats
# register.sh ASSEMBLES concerns/[0-9][0-9]-*.sh into ONE registry module
# <spec>__python-quality.sh under $CLAUDE_CONFIG_DIR/toolu/post-tools.d/,
# prunes its own stale entries + tmp residue, and never touches other plugins.
#
# B.1-B.5 copy register.sh into a scratch dir and plant throwaway fixture
# fragments there — real register.sh logic exercised against real files,
# independent of the repo's own concerns/ contents. The concerns-absent path
# (a plugin home with no concerns/ dir at all) is covered by B.6 with a second
# scratch copy that deliberately omits the dir.

SPEC="python-quality@toolu"
MODULE="${SPEC}__python-quality.sh"

setup() {
  TMP=$(mktemp -d)
  export CLAUDE_CONFIG_DIR="$TMP/cfg"
  REAL_REGISTER="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)/register.sh"
  # Scratch copy of hooks/ with fixture concern fragments planted alongside
  # the copied register.sh, so SRC_DIR="$SELF_DIR/concerns" resolves to real
  # (if throwaway) fragments instead of the not-yet-created repo concerns/.
  PLUGIN_TMP="$TMP/hooks"
  mkdir -p "$PLUGIN_TMP/concerns"
  cp "$REAL_REGISTER" "$PLUGIN_TMP/register.sh"
  REGISTER="$PLUGIN_TMP/register.sh"
  CONCERNS_DIR="$PLUGIN_TMP/concerns"
  REG_DIR="$CLAUDE_CONFIG_DIR/toolu/post-tools.d"
  printf '#!/usr/bin/env bash\necho fragment-a\n' > "$CONCERNS_DIR/10-a.sh"
  printf '#!/usr/bin/env bash\necho fragment-b\n' > "$CONCERNS_DIR/20-b.sh"
}
teardown() { rm -rf "$TMP"; }

# Reproduce the assembly independently of register.sh: in-order concat of the
# numeric-prefixed fragments with a newline after each.
_expected_module() {
  local out="$1"; : > "$out"
  local f
  for f in "$CONCERNS_DIR"/[0-9][0-9]-*.sh; do
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
  echo x > "$REG_DIR/${SPEC}__python-quality.sh.tmp.111"
  touch -t 202601010000 "$REG_DIR/${SPEC}__python-quality.sh.tmp.111"
  echo x > "$REG_DIR/${SPEC}__python-quality.sh.tmp.222"   # fresh — kept
  echo x > "$REG_DIR/other@market__keep.sh.tmp.9"; touch -t 202601010000 "$REG_DIR/other@market__keep.sh.tmp.9"
  run bash "$REGISTER" </dev/null
  [ "$status" -eq 0 ]
  [ ! -f "$REG_DIR/${SPEC}__python-quality.sh.tmp.111" ]
  [ -f "$REG_DIR/${SPEC}__python-quality.sh.tmp.222" ]
  [ -f "$REG_DIR/other@market__keep.sh.tmp.9" ]
}

# --- B.6: concerns/ genuinely absent — no-op, exit 0 ---
@test "register: concerns/ absent (real plugin home) exits 0 and creates no registry entry" {
  BARE_TMP="$TMP/bare-hooks"
  mkdir -p "$BARE_TMP"
  cp "$REAL_REGISTER" "$BARE_TMP/register.sh"
  run bash "$BARE_TMP/register.sh" </dev/null
  [ "$status" -eq 0 ]
  [ -z "$output" ]
  [ ! -d "$REG_DIR" ] || {
    count=$(find "$REG_DIR" -maxdepth 1 -name "${SPEC}__*.sh" | wc -l | tr -d ' ')
    [ "$count" -eq 0 ]
  }
}
