#!/usr/bin/env bats
# catalog.bats — structural invariants for the `design` dispatcher skill.
# Enforces the spec's testable contract: catalog ↔ command files (no orphans),
# SKILL.md size ceiling, zero Node runtime deps, and docs-in-sync (no stale
# `design-review` references). Pure bash + coreutils; no Node, no network.

setup() {
  REPO_ROOT="$(git -C "$BATS_TEST_DIRNAME" rev-parse --show-toplevel)"
  PLUGIN_DIR="$REPO_ROOT/plugins/design"
  SKILL="$PLUGIN_DIR/skills/design/SKILL.md"
  CMD_DIR="$PLUGIN_DIR/references/commands"
}

@test "every catalog command has a command file and vice versa (no orphans)" {
  # Catalog command IDs: the backtick-wrapped token in column 1 of each table row.
  catalog_cmds="$(grep -oE '^\| `[a-z]+`' "$SKILL" | sed -E 's/^\| `([a-z]+)`/\1/' | sort -u)"
  file_cmds="$(for f in "$CMD_DIR"/*.md; do basename "$f" .md; done | sort -u)"
  [ -n "$catalog_cmds" ]
  run diff <(printf '%s\n' "$catalog_cmds") <(printf '%s\n' "$file_cmds")
  [ "$status" -eq 0 ]
}

@test "catalog lists exactly 23 commands" {
  count="$(grep -cE '^\| `[a-z]+`' "$SKILL")"
  [ "$count" -eq 23 ]
}

@test "every command except live is Status: active with Flow/Cites/Output" {
  for f in "$CMD_DIR"/*.md; do
    base="$(basename "$f" .md)"
    [ "$base" = "live" ] && continue
    # Anchor to end-of-line so 'active-soon' or a prose mention can't false-pass.
    grep -qE '\*\*Status:\*\* active *$' "$f" || { echo "not active: $base"; return 1; }
    for s in '^## Flow' '^## Cites' '^## Output'; do
      grep -qE "$s" "$f" || { echo "missing section $s: $base"; return 1; }
    done
  done
}

@test "live is the only deferred command and names an active fallback (no dead end)" {
  grep -qE '\*\*Status:\*\* planned \(deferred\)' "$CMD_DIR/live.md" || { echo "live not deferred"; return 1; }
  grep -q '^## For now' "$CMD_DIR/live.md" || { echo "live has no For-now section"; return 1; }
  grep -A4 '^## For now' "$CMD_DIR/live.md" | grep -qE '/design (critique|audit)' \
    || { echo "live fallback is not an active command"; return 1; }
  # No other command may be a stub.
  stubs="$(grep -lE '\*\*Status:\*\* planned' "$CMD_DIR"/*.md | grep -v '/live.md' || true)"
  [ -z "$stubs" ] || { echo "unexpected stubs: $stubs"; return 1; }
}

@test "SKILL.md catalog Status column matches each command file's Status" {
  # The catalog's Status column is the routing source of truth; assert it never
  # diverges from the actual command file (e.g. catalog says active, file planned).
  for f in "$CMD_DIR"/*.md; do
    base="$(basename "$f" .md)"
    file_status="$(grep -oE '\*\*Status:\*\* .*' "$f" | head -1 | sed -E 's/\*\*Status:\*\* //; s/ *$//')"
    row="$(grep -F "commands/$base.md" "$SKILL")"
    [ -n "$row" ] || { echo "no catalog row for $base"; return 1; }
    case "$row" in
      *"| $file_status |"*) ;;
      *) echo "status mismatch for $base: file='$file_status' not in catalog row"; return 1 ;;
    esac
  done
}

@test "SKILL.md is within the 300-line ceiling" {
  lines="$(grep -c '' "$SKILL")"
  [ "$lines" -le 300 ]
}

@test "zero Node runtime: no .mjs files under the plugin" {
  run bash -c "find '$PLUGIN_DIR' -name '*.mjs' -print -quit"
  [ -z "$output" ]
}

@test "zero Node runtime: package.json only under test fixtures" {
  stray="$(find "$PLUGIN_DIR" -name package.json | grep -v '/scripts/__tests__/fixtures/' || true)"
  [ -z "$stray" ]
}

@test "zero Node runtime: no node/npx invocation in shell scripts" {
  run bash -c "grep -rInE '(^|[^a-zA-Z])(npx|node)[[:space:]]' '$PLUGIN_DIR' --include='*.sh' --include='*.bash' || true"
  [ -z "$output" ]
}

@test "docs in sync: no stale design-review reference in the plugin" {
  # Scope to doc/skill surfaces; --exclude-dir=__tests__ because this test file
  # necessarily contains the search literal (self-reference is not a stale doc).
  run bash -c "grep -rIn --exclude-dir=__tests__ 'design-review' '$PLUGIN_DIR' || true"
  [ -z "$output" ]
}

@test "docs in sync: no stale design-review reference in root README" {
  run bash -c "grep -In 'design-review' '$REPO_ROOT/README.md' || true"
  [ -z "$output" ]
}
