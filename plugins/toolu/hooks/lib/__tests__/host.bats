#!/usr/bin/env bats

LIB="${BATS_TEST_DIRNAME}/../host.sh"

setup() {
  TMP="$(mktemp -d)"
  mkdir -p "$TMP/home"
}

teardown() {
  rm -rf "$TMP"
}

run_host() {
  run env -u PLUGIN_ROOT -u PLUGIN_DATA -u CLAUDE_PLUGIN_ROOT \
    -u CLAUDE_PLUGIN_DATA -u CLAUDE_CONFIG_DIR -u CODEX_HOME \
    -u TOOLU_CONFIG_DIR -u TOOLU_PROJECT_DIR \
    -u TOOLU_PROJECT_CONFIG_DIRNAME -u TOOLU_HOST_OVERRIDE \
    HOME="$TMP/home" "$@"
}

@test "host detection prefers Codex PLUGIN_ROOT over Claude compatibility variables" {
  run_host env PLUGIN_ROOT=/codex CLAUDE_PLUGIN_ROOT=/compat \
    bash -c '. "$1"; toolu_host' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = codex ]
}

@test "host detection defaults to Claude and supports an explicit test override" {
  run_host bash -c '. "$1"; toolu_host' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = claude ]

  run_host env TOOLU_HOST_OVERRIDE=codex bash -c '. "$1"; toolu_host' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = codex ]
}

@test "user config roots are host-native and explicit TOOLU_CONFIG_DIR wins" {
  run_host env TOOLU_HOST_OVERRIDE=claude bash -c '. "$1"; toolu_config_root' _ "$LIB"
  [ "$output" = "$TMP/home/.claude" ]

  run_host env TOOLU_HOST_OVERRIDE=codex bash -c '. "$1"; toolu_config_root' _ "$LIB"
  [ "$output" = "$TMP/home/.codex" ]

  run_host env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex-home" \
    bash -c '. "$1"; toolu_config_root' _ "$LIB"
  [ "$output" = "$TMP/codex-home" ]

  run_host env TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/wrong" TOOLU_CONFIG_DIR="$TMP/explicit" \
    bash -c '. "$1"; toolu_config_root' _ "$LIB"
  [ "$output" = "$TMP/explicit" ]
}

@test "project config and state paths remain isolated by host" {
  run_host env TOOLU_HOST_OVERRIDE=claude TOOLU_PROJECT_DIR="$TMP/repo" \
    bash -c '. "$1"; toolu_project_config; toolu_project_state_dir telemetry' _ "$LIB"
  [ "${lines[0]}" = "$TMP/repo/.claude/toolu.config.json" ]
  [ "${lines[1]}" = "$TMP/repo/.claude/tmp/telemetry" ]

  run_host env TOOLU_HOST_OVERRIDE=codex TOOLU_PROJECT_DIR="$TMP/repo" \
    bash -c '. "$1"; toolu_project_config; toolu_project_state_dir telemetry' _ "$LIB"
  [ "${lines[0]}" = "$TMP/repo/.codex/toolu.config.json" ]
  [ "${lines[1]}" = "$TMP/repo/.codex/tmp/telemetry" ]

  run_host env TOOLU_HOST_OVERRIDE=codex TOOLU_PROJECT_DIR="$TMP/repo" \
    TOOLU_PROJECT_CONFIG_DIRNAME=.test-state \
    bash -c '. "$1"; toolu_project_state_dir telemetry' _ "$LIB"
  [ "$output" = "$TMP/repo/.test-state/tmp/telemetry" ]
}

@test "host-native invocation syntax uses slash for Claude and dollar for Codex" {
  run_host env TOOLU_HOST_OVERRIDE=claude bash -c '. "$1"; toolu_invocation toolu setup' _ "$LIB"
  [ "$output" = /toolu:setup ]

  run_host env TOOLU_HOST_OVERRIDE=codex bash -c '. "$1"; toolu_invocation toolu setup' _ "$LIB"
  [ "$output" = '$toolu:setup' ]

  run_host env TOOLU_HOST_OVERRIDE=claude \
    bash -c '. "$1"; toolu_plugin_install_command toolu@toolu' _ "$LIB"
  [ "$output" = '/plugin install toolu@toolu' ]

  run_host env TOOLU_HOST_OVERRIDE=codex \
    bash -c '. "$1"; toolu_plugin_install_command toolu@toolu' _ "$LIB"
  [ "$output" = 'codex plugin add toolu@toolu' ]
}

@test "Codex plugin snapshot canonicalizes installed plugins atomically" {
  bin="$TMP/bin"
  mkdir -p "$bin"
  cat > "$bin/codex" <<'SH'
#!/usr/bin/env bash
printf '%s\n' '{"installed":[{"pluginId":"toolu@toolu","name":"toolu","marketplaceName":"toolu","installed":true},{"pluginId":"statusline@toolu","name":"statusline","marketplaceName":"toolu","installed":true}],"available":[]}'
SH
  chmod +x "$bin/codex"

  snapshot="$TMP/cache/plugins.json"
  run_host env PATH="$bin:$PATH" TOOLU_HOST_OVERRIDE=codex \
    TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_snapshot_codex_plugins; cat "$TOOLU_CODEX_PLUGIN_SNAPSHOT"' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$(jq -r '.status' "$snapshot")" = ready ]
  [ "$(jq -r '.plugins | join(",")' "$snapshot")" = 'statusline@toolu,toolu@toolu' ]
  [ -z "$(find "$(dirname "$snapshot")" -name '*.tmp.*' -print)" ]
}

@test "Codex plugin snapshot excludes disabled installed plugins" {
  bin="$TMP/bin-disabled"
  mkdir -p "$bin"
  cat > "$bin/codex" <<'SH'
#!/usr/bin/env bash
printf '%s\n' '{"installed":[{"pluginId":"toolu@toolu","installed":true,"enabled":true},{"pluginId":"statusline@toolu","installed":true,"enabled":false},{"pluginId":"git-better@toolu","installed":false,"enabled":true}],"available":[]}'
SH
  chmod +x "$bin/codex"

  snapshot="$TMP/cache/disabled-plugins.json"
  run_host env PATH="$bin:$PATH" TOOLU_HOST_OVERRIDE=codex \
    TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_snapshot_codex_plugins; jq -r ".plugins | join(\",\")" "$TOOLU_CODEX_PLUGIN_SNAPSHOT"' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = 'toolu@toolu' ]
}

@test "Codex plugin snapshot records indeterminate state for missing or malformed CLI output" {
  snapshot="$TMP/cache/plugins.json"
  run_host env PATH=/usr/bin:/bin TOOLU_HOST_OVERRIDE=codex \
    TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_snapshot_codex_plugins; jq -r .status "$TOOLU_CODEX_PLUGIN_SNAPSHOT"' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = indeterminate ]

  bin="$TMP/bin"
  mkdir -p "$bin"
  printf '#!/usr/bin/env bash\nprintf "not json\\n"\n' > "$bin/codex"
  chmod +x "$bin/codex"
  run_host env PATH="$bin:/usr/bin:/bin" TOOLU_HOST_OVERRIDE=codex \
    TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_snapshot_codex_plugins; jq -r .status "$TOOLU_CODEX_PLUGIN_SNAPSHOT"' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = indeterminate ]
}

@test "Codex plugin lookup distinguishes installed, absent, and indeterminate snapshots" {
  snapshot="$TMP/plugins.json"
  printf '%s\n' '{"version":1,"status":"ready","plugins":["toolu@toolu"]}' > "$snapshot"

  run_host env TOOLU_HOST_OVERRIDE=codex TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_codex_plugin_installed toolu@toolu' _ "$LIB"
  [ "$status" -eq 0 ]
  [ "$output" = toolu@toolu ]

  run_host env TOOLU_HOST_OVERRIDE=codex TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_codex_plugin_installed comemory@toolu' _ "$LIB"
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  printf '%s\n' '{"version":1,"status":"ready","plugins":["toolu@another-market"]}' > "$snapshot"
  run_host env TOOLU_HOST_OVERRIDE=codex TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_codex_plugin_installed toolu@toolu' _ "$LIB"
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  printf '%s\n' '{"version":1,"status":"indeterminate","plugins":[]}' > "$snapshot"
  run_host env TOOLU_HOST_OVERRIDE=codex TOOLU_CODEX_PLUGIN_SNAPSHOT="$snapshot" \
    bash -c '. "$1"; toolu_codex_plugin_installed toolu@toolu' _ "$LIB"
  [ "$status" -eq 2 ]
}
