#!/usr/bin/env bats
# Tests for jira.sh dispatcher — global-flag stripping and usage banners.
# Family routing is covered by each family's own suite.

load helpers

setup() { setup_sandbox; }
teardown() { teardown_sandbox; }

@test "dispatch: no args prints usage and exits 1" {
  run bash "$TOOL_DIR/jira.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira"* ]]
  [[ "$output" == *"Families:"* ]]
}

@test "dispatch: unknown family exits 1 naming it" {
  run bash "$TOOL_DIR/jira.sh" bogus
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown family 'bogus'"* ]]
}

@test "dispatch: --api-version <n> is consumed, not treated as family" {
  run bash "$TOOL_DIR/jira.sh" --api-version 2 bogus
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown family 'bogus'"* ]]
}

@test "dispatch: --api-version=<n> form is consumed" {
  run bash "$TOOL_DIR/jira.sh" --api-version=2 bogus
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown family 'bogus'"* ]]
}

@test "dispatch: --lean alone (no family) prints usage" {
  run bash "$TOOL_DIR/jira.sh" --lean
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage: jira"* ]]
}

@test "dispatch: a global flag after the family is still stripped" {
  run bash "$TOOL_DIR/jira.sh" bogus --lean
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown family 'bogus'"* ]]
}
