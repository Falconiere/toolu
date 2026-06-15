#!/usr/bin/env bats
# config.sh — loads ~/.claude/stats.conf into STATS_* vars. Each test writes a
# real config file into an isolated CLAUDE_CONFIG_DIR and parses it; no mocks.

setup() {
  source "${BATS_TEST_DIRNAME}/../scripts/lib/config.sh"
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  mkdir -p "$CLAUDE_CONFIG_DIR"
  CONF="$CLAUDE_CONFIG_DIR/stats.conf"
  unset STATS_PLAN STATS_BILLING STATS_WEEKLY_LIMIT STATS_WINDOW_LIMIT
}

@test "reads the whitelisted keys from the config file" {
  cat > "$CONF" <<'EOF'
plan = max5
billing = both
weekly_limit_tokens = 4000000
window_limit_tokens = 300000
EOF
  stats_load_config
  [ "$STATS_PLAN" = "max5" ]
  [ "$STATS_BILLING" = "both" ]
  [ "$STATS_WEEKLY_LIMIT" = "4000000" ]
  [ "$STATS_WINDOW_LIMIT" = "300000" ]
}

@test "ignores comments, blank lines, and unknown keys" {
  cat > "$CONF" <<'EOF'
# this is a comment
plan=pro

bogus_key=should_be_ignored
not even a kv line
EOF
  stats_load_config
  [ "$STATS_PLAN" = "pro" ]
  [ -z "${STATS_BILLING:-}" ]
}

@test "does NOT execute command substitution in a value (no eval)" {
  printf 'plan=$(touch %s/PWNED)\n' "$BATS_TEST_TMPDIR" > "$CONF"
  stats_load_config
  [ ! -e "$BATS_TEST_TMPDIR/PWNED" ]              # nothing executed
  [ "$STATS_PLAN" = "\$(touch $BATS_TEST_TMPDIR/PWNED)" ]   # stored as a literal
}

@test "an already-set env var (or flag) wins over the config file" {
  export STATS_PLAN=pro
  printf 'plan=max20\n' > "$CONF"
  stats_load_config
  [ "$STATS_PLAN" = "pro" ]                       # config does not clobber it
}

@test "a missing config file is a silent no-op" {
  rm -f "$CONF"
  run stats_load_config
  [ "$status" -eq 0 ]
  [ -z "${STATS_PLAN:-}" ]
}

@test "a trailing inline comment is stripped from a value" {
  printf 'plan=max5   # my plan\n' > "$CONF"
  stats_load_config
  [ "$STATS_PLAN" = "max5" ]
}
