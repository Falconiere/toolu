#!/usr/bin/env bats

SCRIPT="${BATS_TEST_DIRNAME}/../pre-compact.sh"

@test "pre-compact is silent for Codex auto-compaction input" {
  run bash -c "printf '%s' '{\"hook_event_name\":\"PreCompact\",\"trigger\":\"auto\"}' | '$SCRIPT' 2>&1"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pre-compact is silent with empty stdin" {
  run bash -c "'$SCRIPT' < /dev/null 2>&1"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "pre-compact is silent when disabled" {
  config_dir="$BATS_TEST_TMPDIR/config"
  project_dir="$BATS_TEST_TMPDIR/project"
  mkdir -p "$config_dir" "$project_dir"
  printf '%s\n' '{"hooks":{"pre-compact":false}}' > "$config_dir/toolu.config.json"

  run bash -c "printf '%s' '{\"hook_event_name\":\"PreCompact\",\"trigger\":\"auto\"}' | env TOOLU_CONFIG_DIR='$config_dir' TOOLU_PROJECT_DIR='$project_dir' '$SCRIPT' 2>&1"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}
