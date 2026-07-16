#!/usr/bin/env bats
# The SessionStart mandatory-proactive-tool-use block: when the comemory /
# ast-grep plugins are installed (per the registry) AND their binary is on
# PATH, session-start must emit a hard, non-optional mandate to use them
# proactively. Absent the plugin (registry parsed, spec not present), no
# mandate. Drives the REAL entrypoint with a synthetic installed-plugins
# registry; binary presence is real (skips when the tool is not installed).
#
# exa-search / context7 have no binary on PATH — their presence signal is the
# CLI wrapper their own SessionStart hooks publish under
# ${CLAUDE_CONFIG_DIR:-$HOME/.claude}/<plugin>/search.sh. exa-search's mandate
# additionally requires EXA_API_KEY in the environment (the wrapper hard-fails
# without it); context7 is keyless and fires on plugin + wrapper alone.
#
# comemory's mandate is OPT-IN: it fires only after /comemory:setup has written
# the `comemory.setup_done` flag into toolu.config.json. Before that the block
# emits a one-line /comemory:setup nudge instead — but only when jq is present
# (the flag is unreadable without jq, so the nudge is suppressed to avoid a
# perpetual "run setup" on jq-less hosts) and setup_done is not explicitly
# false (an explicit false is a deliberate opt-out, not an unanswered nudge).

setup() {
  PLUGINS_DIR="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../.." && pwd)"
  ENTRY="$PLUGINS_DIR/toolu/hooks/session-start.sh"
  TMP=$(mktemp -d)
  REG="$TMP/installed_plugins.json"
}
teardown() { [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"; }

# Write a project (and, since HOME=$TMP, user) toolu.config.json with the given
# JSON body. Both config layers resolve to $TMP/.claude/toolu.config.json under
# the test env, so one file feeds the merge.
_write_config() {
  mkdir -p "$TMP/.claude"
  printf '%s' "$1" > "$TMP/.claude/toolu.config.json"
}

# Run session-start from an empty dir (no toolu manifest in PROJECT_ROOT,
# so the dep-warning block stays quiet) with a synthetic plugins registry.
# CLAUDE_PROJECT_DIR pins the project-config root at $TMP so _write_config's
# toolu.config.json is the config the loader reads. CLAUDE_CONFIG_DIR is
# pinned to $TMP/.claude (identical to the HOME=$TMP default — explicit so a
# dev shell's real CLAUDE_CONFIG_DIR can't leak in) and EXA_API_KEY is
# blanked for the same reason. Extra VAR=value args pass through to env for
# per-test overrides: `_run_entry EXA_API_KEY=x`.
_run_entry() {
  ( cd "$TMP" && env CLAUDE_PLUGINS_REGISTRY="$REG" HOME="$TMP" \
      CLAUDE_PROJECT_DIR="$TMP" CLAUDE_CONFIG_DIR="$TMP/.claude" \
      EXA_API_KEY= "$@" \
      bash "$ENTRY" <<<'{"hook_event_name":"SessionStart","source":"startup"}' )
}

# Publish a stub CLI wrapper at the stable config-dir path the exa-search /
# context7 plugins' SessionStart hooks symlink to — the mandate block treats
# the wrapper file (executable) as the plugin's presence signal.
_publish_wrapper() {
  mkdir -p "$TMP/.claude/$1"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$TMP/.claude/$1/search.sh"
  chmod +x "$TMP/.claude/$1/search.sh"
}

# Run the entry under a sanitized PATH that LACKS jq but carries the coreutils
# the hook needs (mirrors the jq-masking approach in lib/__tests__/config.bats):
# symlink a known-good toolset into a stub bin dir, then env -i with that PATH.
_run_entry_no_jq() {
  local stub="$TMP/nojq-bin"
  mkdir -p "$stub"
  for t in bash sh env git grep sed awk tr head cut cat printf basename dirname mktemp rm mkdir comemory ast-grep sg; do
    src=$(command -v "$t" 2>/dev/null) && ln -sf "$src" "$stub/$t"
  done
  ( cd "$TMP" && env -i \
      PATH="$stub" \
      HOME="$TMP" \
      CLAUDE_PLUGINS_REGISTRY="$REG" \
      CLAUDE_PROJECT_DIR="$TMP" \
      bash "$ENTRY" <<<'{"hook_event_name":"SessionStart","source":"startup"}' )
}

@test "mandate: comemory mandate fires when plugin active + binary present + setup_done true" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$REG"
  _write_config '{"comemory":{"setup_done":true}}'
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "MANDATORY"
  echo "$output" | grep -q 'comemory.sh search'
  echo "$output" | grep -q "do NOT ask permission"
  # The opt-in path replaces the nudge, never both.
  ! echo "$output" | grep -q '/comemory:setup'
}

@test "mandate: NO comemory mandate when setup_done absent — /comemory:setup nudge instead" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$REG"
  # No comemory.setup_done flag written.
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'comemory.sh search'
  echo "$output" | grep -q '/comemory:setup'
  echo "$output" | grep -q 'comemory detected but not enabled'
}

@test "mandate: setup_done explicitly false — NO mandate and NO nudge (deliberate opt-out)" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$REG"
  _write_config '{"comemory":{"setup_done":false}}'
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'comemory.sh search'
  # Explicit false is an answered question — nudging again would nag.
  ! echo "$output" | grep -q '/comemory:setup'
}

@test "mandate: NO comemory mandate when skills.comemory == false (even with setup_done true)" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$REG"
  _write_config '{"skills":{"comemory":false},"comemory":{"setup_done":true}}'
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'comemory.sh search'
  # skills-disabled gates BOTH the mandate and the nudge.
  ! echo "$output" | grep -q '/comemory:setup'
}

@test "mandate: jq masked — NEITHER comemory mandate NOR /comemory:setup nudge" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{}}}' > "$REG"
  _write_config '{"comemory":{"setup_done":true}}'
  # Confirm the stub PATH really hides jq before asserting on absence.
  run _run_entry_no_jq
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'comemory.sh search'
  ! echo "$output" | grep -q '/comemory:setup'
}

@test "mandate: ast-grep plugin installed + binary present emits a structural-search mandate" {
  command -v ast-grep >/dev/null 2>&1 || command -v sg >/dev/null 2>&1 || skip "ast-grep binary not installed"
  printf '%s' '{"plugins":{"ast-grep@toolu":{}}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "MANDATORY"
  echo "$output" | grep -q 'ast-grep run --pattern'
  echo "$output" | grep -q "FALLBACK ONLY"
}

@test "mandate: no comemory mandate when the plugin is definitively absent" {
  printf '%s' '{"plugins":{}}' > "$REG"
  _write_config '{"comemory":{"setup_done":true}}'
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'comemory.sh search'
  # Plugin absent also gates the nudge.
  ! echo "$output" | grep -q '/comemory:setup'
}

@test "mandate: no ast-grep mandate when the plugin is definitively absent" {
  printf '%s' '{"plugins":{}}' > "$REG"
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'ast-grep run --pattern'
}

@test "mandate: both mandates fire under one MANDATORY header (comemory opted in)" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  command -v ast-grep >/dev/null 2>&1 || command -v sg >/dev/null 2>&1 || skip "ast-grep binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{},"ast-grep@toolu":{}}}' > "$REG"
  _write_config '{"comemory":{"setup_done":true}}'
  run _run_entry
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | grep -c "MANDATORY — proactive plugin use")" -eq 1 ]
  echo "$output" | grep -q 'comemory.sh search'
  echo "$output" | grep -q 'ast-grep run --pattern'
  # Mandates propagate to nested subagents.
  echo "$output" | grep -q "Propagation"
  echo "$output" | grep -q "bind EVERY agent"
}

@test "mandate: exa-search mandate fires when plugin active + wrapper published + EXA_API_KEY set" {
  printf '%s' '{"plugins":{"exa-search@toolu":{}}}' > "$REG"
  _publish_wrapper exa-search
  run _run_entry EXA_API_KEY=test-key
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "MANDATORY"
  echo "$output" | grep -q 'exa-search/search.sh'
  echo "$output" | grep -q "FALLBACK ONLY"
}

@test "mandate: NO exa-search mandate when EXA_API_KEY is unset (wrapper would hard-fail)" {
  printf '%s' '{"plugins":{"exa-search@toolu":{}}}' > "$REG"
  _publish_wrapper exa-search
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'exa-search/search.sh'
}

@test "mandate: NO exa-search mandate when the wrapper is not published" {
  printf '%s' '{"plugins":{"exa-search@toolu":{}}}' > "$REG"
  # No _publish_wrapper — first-session race or broken publish hook.
  run _run_entry EXA_API_KEY=test-key
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'exa-search/search.sh'
}

@test "mandate: NO exa-search mandate when skills.exa-search == false" {
  printf '%s' '{"plugins":{"exa-search@toolu":{}}}' > "$REG"
  _publish_wrapper exa-search
  _write_config '{"skills":{"exa-search":false}}'
  run _run_entry EXA_API_KEY=test-key
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'exa-search/search.sh'
}

@test "mandate: context7 mandate fires when plugin active + wrapper published (no API key needed)" {
  printf '%s' '{"plugins":{"context7@toolu":{}}}' > "$REG"
  _publish_wrapper context7
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "MANDATORY"
  echo "$output" | grep -q 'context7/search.sh'
  echo "$output" | grep -q 'docs <id> <query>'
}

@test "mandate: NO context7 mandate when the wrapper is not published" {
  printf '%s' '{"plugins":{"context7@toolu":{}}}' > "$REG"
  # No _publish_wrapper — first-session race or broken publish hook.
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'context7/search.sh'
}

@test "mandate: NO context7 mandate when the plugin is definitively absent (wrapper published)" {
  printf '%s' '{"plugins":{}}' > "$REG"
  _publish_wrapper context7
  _publish_wrapper exa-search
  run _run_entry EXA_API_KEY=test-key
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'context7/search.sh'
  ! echo "$output" | grep -q 'exa-search/search.sh'
}

@test "mandate: NO context7 mandate when skills.context7 == false" {
  printf '%s' '{"plugins":{"context7@toolu":{}}}' > "$REG"
  _publish_wrapper context7
  _write_config '{"skills":{"context7":false}}'
  run _run_entry
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'context7/search.sh'
}

@test "mandate: all four mandates fire under one MANDATORY header" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  command -v ast-grep >/dev/null 2>&1 || command -v sg >/dev/null 2>&1 || skip "ast-grep binary not installed"
  printf '%s' '{"plugins":{"comemory@toolu":{},"ast-grep@toolu":{},"exa-search@toolu":{},"context7@toolu":{}}}' > "$REG"
  _write_config '{"comemory":{"setup_done":true}}'
  _publish_wrapper exa-search
  _publish_wrapper context7
  run _run_entry EXA_API_KEY=test-key
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | grep -c "MANDATORY — proactive plugin use")" -eq 1 ]
  echo "$output" | grep -q 'comemory.sh search'
  echo "$output" | grep -q 'ast-grep run --pattern'
  echo "$output" | grep -q 'exa-search/search.sh'
  echo "$output" | grep -q 'context7/search.sh'
}

@test "mandate: indeterminate registry fails open — comemory mandate still fires when opted in" {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
  rm -f "$REG"   # registry absent → toolu_plugin_active fails open
  _write_config '{"comemory":{"setup_done":true}}'
  run _run_entry
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'comemory.sh search'
}
