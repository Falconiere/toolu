#!/usr/bin/env bats
# Tests for hooks/lib/docs-sync-config.sh — default glob sets + project override.
# Isolated from the real user config via TOOLU_CONFIG_DIR (empty) +
# TOOLU_PROJECT_DIR (a throwaway project root).

LIB="$(cd "$(dirname "$BATS_TEST_FILENAME")/.." && pwd)"

setup() {
  TMP="$(mktemp -d)"
  EMPTY_CFG_DIR="$TMP/agent"      # contains no toolu.config.json -> no user override
  PROJECT="$TMP/project"
  mkdir -p "$EMPTY_CFG_DIR" "$PROJECT/.claude"
}

teardown() {
  [ -n "${TMP:-}" ] && rm -rf "$TMP"
}

# Source the lib and call a function in an isolated env (one process, no cache bleed).
ds() {
  run env TOOLU_CONFIG_DIR="$EMPTY_CFG_DIR" TOOLU_PROJECT_DIR="$PROJECT" \
    bash -c ". '$LIB/docs-sync-config.sh'; $1"
}

@test "docs_sync_surfaces returns the built-in defaults when no config" {
  ds docs_sync_surfaces
  [ "$status" -eq 0 ]
  echo "$output" | grep -qFx 'README.md'
  echo "$output" | grep -qFx 'docs/*.md'
  echo "$output" | grep -qFx '*/SKILL.md'
}

@test "docs_sync_code_surfaces returns the built-in defaults when no config" {
  ds docs_sync_code_surfaces
  [ "$status" -eq 0 ]
  echo "$output" | grep -qFx '*.ts'
  echo "$output" | grep -qFx '*.sh'
  echo "$output" | grep -qFx '*plugin.json'
}

@test "docs_sync_surface_excludes defaults to carving out release notes" {
  ds docs_sync_surface_excludes
  [ "$status" -eq 0 ]
  echo "$output" | grep -qFx 'docs/releases/*'
}

@test "project config overrides surfaces, dropping the defaults" {
  printf '%s' '{"docsSync":{"surfaces":["CUSTOM.md","wiki/*.md"]}}' \
    > "$PROJECT/.claude/toolu.config.json"
  ds docs_sync_surfaces
  [ "$status" -eq 0 ]
  echo "$output" | grep -qFx 'CUSTOM.md'
  echo "$output" | grep -qFx 'wiki/*.md'
  ! echo "$output" | grep -qFx 'README.md'
}

@test "overriding codeSurfaces leaves surfaces on the default" {
  printf '%s' '{"docsSync":{"codeSurfaces":["*.py"]}}' \
    > "$PROJECT/.claude/toolu.config.json"
  ds docs_sync_code_surfaces
  [ "$status" -eq 0 ]
  echo "$output" | grep -qFx '*.py'
  ! echo "$output" | grep -qFx '*.ts'
  ds docs_sync_surfaces
  echo "$output" | grep -qFx 'README.md'
}

# ── the main memory is a protected surface ─────────────────────────────────
#
# AGENTS.md is the single source of truth for how agents work in this repo, and
# it was not a docs-sync surface — so a change that made it stale raised nothing.
# That is not hypothetical: a branch changed how the test suite runs and left
# AGENTS.md describing the old commands, and the gate said nothing.

@test "docs_sync_surfaces includes the agent instruction files" {
  ds docs_sync_surfaces
  [ "$status" -eq 0 ]
  echo "$output" | grep -qFx 'AGENTS.md'
  echo "$output" | grep -qFx 'CLAUDE.md'
}

@test "docs_sync_surfaces covers nested agent instruction files" {
  # A plugin or subproject can carry its own; those count too.
  ds docs_sync_surfaces
  echo "$output" | grep -qFx '*/AGENTS.md'
  echo "$output" | grep -qFx '*/CLAUDE.md'
}

@test "the original surfaces still hold alongside the new ones" {
  ds docs_sync_surfaces
  echo "$output" | grep -qFx 'README.md'
  echo "$output" | grep -qFx '*/README.md'
  echo "$output" | grep -qFx 'docs/*.md'
  echo "$output" | grep -qFx '*/SKILL.md'
}

@test "an explicit surfaces override still drops the agent files" {
  # Override semantics are unchanged: naming surfaces replaces the defaults
  # wholesale rather than appending to them.
  printf '%s' '{"docsSync":{"surfaces":["CUSTOM.md"]}}' \
    > "$PROJECT/.claude/toolu.config.json"
  ds docs_sync_surfaces
  echo "$output" | grep -qFx 'CUSTOM.md'
  ! echo "$output" | grep -qFx 'AGENTS.md'
}

@test "docs_sync_surfaces covers plugin workflow docs" {
  # A plugin's workflows/*.md is prose that describes behaviour — pr-babysit's
  # babysit.md is the whole contract for how that plugin runs. It was not a
  # surface, so the gate fired "code changed, no docs" on the very push that
  # rewrote it.
  ds docs_sync_surfaces
  echo "$output" | grep -qFx '*/workflows/*.md'
}
