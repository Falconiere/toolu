#!/usr/bin/env bats
# conventions-cache — cache reuse/invalidation + prose persistence, via the gb
# dispatcher. Temp repo + temp TOOLU_CONFIG_DIR. Real git, no mocks. AC#9.

setup() {
  GB="$BATS_TEST_DIRNAME/../git-better.sh"
  export TOOLU_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  REPO="$BATS_TEST_TMPDIR/repo"
  mkdir -p "$REPO"; cd "$REPO"
  git init -q
  git config user.email t@example.com; git config user.name tester
  printf 'export const a = 1\n' > src.ts
  git add -A
  git commit -qm "feat(core): init (#1)"
}

@test "cache reuse: two reads → identical generated_at (no recompute) (AC#9)" {
  g1="$(bash "$GB" conventions --json | jq -r .generated_at)"
  g2="$(bash "$GB" conventions --json | jq -r .generated_at)"
  [ -n "$g1" ]
  [ "$g1" = "$g2" ]
}

@test "invalidation: changed declared file → new source_hash → recompute (AC#9)" {
  h1="$(bash "$GB" conventions --json | jq -r .source_hash)"
  printf 'commit template\n' > .gitmessage   # a declared convention file
  h2="$(bash "$GB" conventions --json | jq -r .source_hash)"
  [ "$h1" != "$h2" ]
}

@test "--refresh: forces recompute (new generated_at) (AC#9)" {
  g1="$(bash "$GB" conventions --json | jq -r .generated_at)"
  sleep 1
  g2="$(bash "$GB" conventions --refresh --json | jq -r .generated_at)"
  [ "$g1" != "$g2" ]
}

@test "prose: CONTRIBUTING.md pending → save via stdin → distilled stored, pending empty" {
  printf 'Squash commits before merge.\n' > CONTRIBUTING.md
  bash "$GB" conventions --json | jq -e '.prose_pending | index("CONTRIBUTING.md")'
  printf 'Squash before merge; conventional title.' | bash "$GB" conventions --save-prose CONTRIBUTING.md
  run bash "$GB" conventions --json
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.prose_distilled["CONTRIBUTING.md"].rules == "Squash before merge; conventional title."'
  echo "$output" | jq -e '.prose_pending == []'
}
