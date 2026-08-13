#!/usr/bin/env bats

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../../../.." && pwd)"

@test "Codex command-equivalent skills are packaged under their plugin namespaces" {
  expected=(
    "plugins/toolu/skills/commit/SKILL.md"
    "plugins/toolu/skills/review-and-commit/SKILL.md"
    "plugins/comemory/skills/setup/SKILL.md"
    "plugins/statusline/skills/status/SKILL.md"
    "plugins/pr-babysit/skills/babysit/SKILL.md"
    "plugins/toolu/skills/setup/SKILL.md"
  )

  for path in "${expected[@]}"; do
    [ -f "$ROOT/$path" ] || {
      echo "missing $path"
      return 1
    }
  done
}

@test "Claude commit commands and Codex commit skills point to one canonical workflow each" {
  pairs=(
    "toolu|commands/commit.md|skills/commit/SKILL.md|workflows/commit.md"
    "toolu|commands/review-and-commit.md|skills/review-and-commit/SKILL.md|workflows/review-and-commit.md"
    "comemory|commands/setup.md|skills/setup/SKILL.md|workflows/setup.md"
  )

  for pair in "${pairs[@]}"; do
    IFS='|' read -r plugin command skill workflow <<<"$pair"
    [ -f "$ROOT/plugins/$plugin/$command" ]
    [ -f "$ROOT/plugins/$plugin/$skill" ]
    [ -f "$ROOT/plugins/$plugin/$workflow" ]
    grep -Fq "$workflow" "$ROOT/plugins/$plugin/$command"
    grep -Fq "$workflow" "$ROOT/plugins/$plugin/$skill"
  done
}

@test "Codex command skill frontmatter has names and trigger descriptions" {
  for skill in \
    "$ROOT/plugins/toolu/skills/commit/SKILL.md" \
    "$ROOT/plugins/toolu/skills/review-and-commit/SKILL.md" \
    "$ROOT/plugins/comemory/skills/setup/SKILL.md" \
    "$ROOT/plugins/statusline/skills/status/SKILL.md" \
    "$ROOT/plugins/pr-babysit/skills/babysit/SKILL.md" \
    "$ROOT/plugins/toolu/skills/setup/SKILL.md"; do
    name=$(sed -n 's/^name: //p' "$skill")
    description=$(sed -n 's/^description: //p' "$skill")
    [[ "$name" =~ ^[a-z0-9-]+$ ]]
    [[ "$description" == Use\ when* ]]
  done
}
