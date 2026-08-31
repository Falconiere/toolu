#!/usr/bin/env bats
# gb wrapper — real git repos (temp), no mocks. Spec AC #1,2,3,15.

setup() {
  GB="$BATS_TEST_DIRNAME/../git-better.sh"
  REPO="$BATS_TEST_TMPDIR/repo"
  mkdir -p "$REPO"
  cd "$REPO"
  git init -q
  git config user.email t@example.com
  git config user.name tester
  printf 'export const a = 1\n' > src.ts
  printf 'lockfile v1\n' > bun.lock
  git add -A
  git commit -qm "init"
}

@test "status: -sb short format, no ANSI color (AC#1)" {
  printf 'export const a = 2\n' > src.ts
  run bash "$GB" status
  [ "$status" -eq 0 ]
  [[ "$output" != *$'\033'* ]]      # no escape codes
  [[ "$output" == *" M src.ts"* ]]  # short format
}

@test "diff (bare): --stat excludes lockfile, keeps source (AC#2)" {
  printf 'export const a = 2\n' > src.ts
  printf 'lockfile v2\n' > bun.lock
  run bash "$GB" diff
  [ "$status" -eq 0 ]
  [[ "$output" == *"src.ts"* ]]
  [[ "$output" != *"bun.lock"* ]]
}

@test "diff <path>: full hunks for that path (AC#2)" {
  printf 'export const a = 2\n' > src.ts
  run bash "$GB" diff src.ts
  [ "$status" -eq 0 ]
  [[ "$output" == *"diff --git"* ]]
  [[ "$output" == *"export const a = 2"* ]]
}

@test "diff --full: all files incl lockfile (AC#2)" {
  printf 'export const a = 2\n' > src.ts
  printf 'lockfile v2\n' > bun.lock
  run bash "$GB" diff --full
  [ "$status" -eq 0 ]
  [[ "$output" == *"src.ts"* ]]
  [[ "$output" == *"bun.lock"* ]]
}

@test "diff --cached: forwarded verbatim to staged diff (AC#15)" {
  printf 'export const a = 2\n' > src.ts
  git add src.ts
  run bash "$GB" diff --cached
  [ "$status" -eq 0 ]
  [[ "$output" == *"diff --git"* ]]
  [[ "$output" == *"src.ts"* ]]
}

@test "log: capped at 20 by default (AC#3)" {
  for i in $(seq 1 22); do printf 'l%s\n' "$i" >> src.ts; git commit -qam "c$i"; done
  run bash "$GB" log
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | wc -l | tr -d ' ')" -eq 20 ]
}

@test "log -n: arg overrides the default cap (AC#3)" {
  for i in $(seq 1 5); do printf 'l%s\n' "$i" >> src.ts; git commit -qam "c$i"; done
  run bash "$GB" log -n 3
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | wc -l | tr -d ' ')" -eq 3 ]
}

@test "unknown subcommand: usage + exit 1" {
  run bash "$GB" frobnicate
  [ "$status" -eq 1 ]
  [[ "$output" == *"unknown subcommand"* ]]
}
