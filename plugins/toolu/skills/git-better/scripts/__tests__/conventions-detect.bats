#!/usr/bin/env bats
# conventions-detect — real git, no mocks. Inference tests build a HERMETIC temp
# repo with known conventional history + branches + release.yml, so they don't
# depend on the ambient checkout (CI uses a shallow depth-1 clone → no history /
# branches). Plus a temp empty-history repo and a PATH-masked (gh-absent) run.

setup() {
  DETECT="$BATS_TEST_DIRNAME/../lib/conventions-detect.sh"
  ROOT="$(cd "$BATS_TEST_DIRNAME/../../../../../.." && pwd)"
}

# Build a hermetic repo at $1: conventional commits (feat/fix/chore/test + a
# chore(release) + (#N) suffixes), type/kebab branches, and a release.yml.
mkrich() {
  local d="$1"
  mkdir -p "$d/.github/workflows"
  ( cd "$d"
    git init -q
    git config user.email t@example.com; git config user.name tester
    printf 'release stub\n' > .github/workflows/release.yml
    printf 'x\n' > f; git add -A
    git commit -qm "feat(core): init (#1)"
    git commit -qm "fix(api): bug (#2)" --allow-empty
    git commit -qm "chore(deps): bump (#3)" --allow-empty
    git commit -qm "test(core): cover (#4)" --allow-empty
    git commit -qm "chore(release): v1.2.0 (#5)" --allow-empty
    git branch feat/a; git branch fix/b; git branch chore/c; git branch release/v1.2.0
  )
}

@test "commit_format inferred as conventional-commits + scope + (#N) (AC#4)" {
  REPO="$BATS_TEST_TMPDIR/rich"; mkrich "$REPO"
  run bash "$DETECT" "$REPO"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.commit_format.convention == "conventional-commits"'
  echo "$output" | jq -e '.commit_format.scope == "used"'
  echo "$output" | jq -e '.commit_format.pr_suffix == "(#N)"'
  echo "$output" | jq -e '.commit_format.types | (index("feat") and index("fix") and index("chore") and index("test"))'
}

@test "branch_naming inferred as type/kebab (AC#5)" {
  REPO="$BATS_TEST_TMPDIR/rich"; mkrich "$REPO"
  run bash "$DETECT" "$REPO"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.branch_naming.pattern == "type/kebab"'
  echo "$output" | jq -e '.branch_naming.prefixes | (index("feat") and index("fix") and index("chore") and index("release"))'
}

@test "release tooling + version commit form (AC#7)" {
  REPO="$BATS_TEST_TMPDIR/rich"; mkrich "$REPO"
  run bash "$DETECT" "$REPO"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.release.tooling | index("release.yml-workflow")'
  echo "$output" | jq -e '.release.version_commit == "chore(release): vX"'
}

@test "this repo: declares nothing → null template, empty prose_pending, exit 0 (AC#6)" {
  run bash "$DETECT" "$ROOT"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.pr.template_path == null'
  echo "$output" | jq -e '.prose_pending == []'
}

@test "gh absent (PATH masked): gh_available false, recent_titles empty (AC#8)" {
  mkdir -p "$BATS_TEST_TMPDIR/bin"
  # Symlink every PATH binary EXCEPT gh — real binaries, gh genuinely absent.
  IFS=: read -ra dirs <<< "$PATH"
  for d in "${dirs[@]}"; do
    [ -d "$d" ] || continue
    for f in "$d"/*; do
      n="$(basename "$f")"
      [ "$n" = "gh" ] && continue
      [ -e "$BATS_TEST_TMPDIR/bin/$n" ] || ln -s "$f" "$BATS_TEST_TMPDIR/bin/$n" 2>/dev/null || true
    done
  done
  PATH="$BATS_TEST_TMPDIR/bin" run bash "$DETECT" "$ROOT"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.gh_available == false'
  echo "$output" | jq -e '.pr.recent_titles == []'
}

@test "empty-history repo: convention + branch pattern unknown, exit 0 (AC#14)" {
  REPO="$BATS_TEST_TMPDIR/empty"
  mkdir -p "$REPO"; cd "$REPO"
  git init -q
  git config user.email t@example.com; git config user.name tester
  run bash "$DETECT" "$REPO"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.commit_format.convention == "unknown"'
  echo "$output" | jq -e '.branch_naming.pattern == "unknown"'
}

# ── 50-commit window: recency tracks CURRENT convention (OQ3 resolution) ──────
@test "recency: recent conventional commits over older noise → conventional (50-window)" {
  REPO="$BATS_TEST_TMPDIR/adopt"
  mkdir -p "$REPO"; cd "$REPO"
  git init -q; git config user.email t@example.com; git config user.name tester
  printf 'x\n' > f; git add -A
  # 25 older non-conventional commits, then 30 recent conventional ones.
  for i in $(seq 1 25); do printf 'a%s\n' "$i" >> f; git commit -qam "update stuff $i"; done
  for i in $(seq 1 30); do printf 'b%s\n' "$i" >> f; git commit -qam "feat(core): change $i"; done
  run bash "$DETECT" "$REPO"
  [ "$status" -eq 0 ]
  # window = 50 newest = 30 conventional + 20 noise → majority conventional;
  # the 5 oldest commits fall outside the window and don't drag it to unknown.
  echo "$output" | jq -e '.commit_format.convention == "conventional-commits"'
}

@test "no false positive: all non-conventional history → unknown" {
  REPO="$BATS_TEST_TMPDIR/messy"
  mkdir -p "$REPO"; cd "$REPO"
  git init -q; git config user.email t@example.com; git config user.name tester
  printf 'x\n' > f; git add -A
  for i in $(seq 1 20); do printf 'c%s\n' "$i" >> f; git commit -qam "wip $i"; done
  run bash "$DETECT" "$REPO"
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.commit_format.convention == "unknown"'
}
