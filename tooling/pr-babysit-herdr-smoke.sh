#!/usr/bin/env bash
# Real-session smoke for pr-babysit's herdr fixer dispatch (spec AC-10).
#
# Runs the shipped route-fix.sh and dispatch-fix.sh against a live herdr server
# and a real Claude Code fixer, inside a throwaway local git topology (a bare
# "origin" plus a clone). Nothing is pushed to GitHub. Proves: start creates
# the pr-babysit/<slot> herdr worktree and starts the fixer; a second start is
# refused (fixer_running); wait settles two tier groups in order (the second
# launched by wait itself), `done` with exactly two commits touching only the
# smoke file; cleanup leaves no worktree, branch or agent behind.
#
# Manual runner (needs herdr, claude, jq, git) — like tooling/codex-smoke.sh.
# PB_SMOKE_MODEL / PB_SMOKE_EFFORT pick the fixer model (default haiku / low).
set -euo pipefail

ROOT="$(cd "${BASH_SOURCE%/*}/.." && pwd -P)"
S="$ROOT/plugins/pr-babysit/scripts"
SNAP="$S/__tests__/fixtures/snapshots/toolu-165.json"
MODEL="${PB_SMOKE_MODEL:-haiku}"
EFFORT="${PB_SMOKE_EFFORT:-low}"

fail() { printf 'pr-babysit-herdr-smoke: FAIL: %s\n' "$*" >&2; exit 1; }
step() { printf 'pr-babysit-herdr-smoke: %s\n' "$*"; }

for c in herdr claude jq git; do command -v "$c" >/dev/null 2>&1 || fail "$c is required"; done
herdr workspace list >/dev/null 2>&1 || fail "herdr server is not reachable"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/pb-herdr-smoke.XXXXXX")"
TMP="$(cd "$TMP" && pwd -P)"
STATE="$TMP/pr-babysit-local-pb-smoke-1.json"
CLONE="$TMP/pb-smoke"

finish() {
  local rc=$?
  if [ -f "$STATE" ] && [ "$(jq -r '.herdrWorktree // null | type' "$STATE" 2>/dev/null)" = object ]; then
    bash "$S/dispatch-fix.sh" cleanup --state-file "$STATE" >/dev/null 2>&1 || true
  fi
  # herdr opened a primary workspace for the throwaway clone; close only that one.
  herdr workspace list 2>/dev/null | jq -r --arg t "$TMP" '.result.workspaces[]? | select((.worktree.repo_root // "") | startswith($t)) | .workspace_id' \
    | while IFS= read -r ws; do herdr workspace close "$ws" >/dev/null 2>&1 || true; done
  rm -rf "$TMP"
  [ "$rc" -eq 0 ] && step "PASS" || step "exit $rc"
}
trap finish EXIT

# ---- a throwaway "PR": bare origin, main + feat/smoke, a clone on the PR branch
git init --quiet --bare --initial-branch=main "$TMP/origin.git"
git clone --quiet "$TMP/origin.git" "$CLONE" 2>/dev/null
git -C "$CLONE" config user.email smoke@example.invalid
git -C "$CLONE" config user.name "pr-babysit smoke"
printf 'hello\n' >"$CLONE/smoke.txt"
git -C "$CLONE" add smoke.txt && git -C "$CLONE" commit --quiet -m "chore: seed"
git -C "$CLONE" push --quiet origin main
git -C "$CLONE" checkout --quiet -b feat/smoke
printf 'the PR change\n' >>"$CLONE/smoke.txt"
git -C "$CLONE" commit --quiet -am "feat: smoke change"
git -C "$CLONE" push --quiet -u origin feat/smoke

# ---- slot state from a real tick, re-identified as local/pb-smoke#1
jq '.repo = "local/pb-smoke" | .number = 1 | .pr.state = "OPEN"' "$SNAP" >"$TMP/snap.json"
bash "$S/babysit-tick.sh" --repo local/pb-smoke --pr 1 --state-file "$STATE" --snapshot-in "$TMP/snap.json" >/dev/null
[ "$(jq -r .slot "$STATE")" = local-pb-smoke-1 ] || fail "unexpected slot $(jq -r .slot "$STATE")"

# ---- two Fix items in two tiers, routed to real Claude fixers
jq -n '{round: 1, items: [
  {id: "smoke-1", kind: "conversation", path: "smoke.txt", severity: "medium",
   task: "Append exactly one line reading: fixed by pr-babysit smoke — to the end of smoke.txt, then commit only that change with the message: fix(smoke): append line. Change nothing else and run no tests (this file has none)."},
  {id: "smoke-2", kind: "conversation", path: "smoke.txt", severity: "nit",
   task: "Fix the typo on the first line of smoke.txt: it must read hello world instead of hello. Commit only that change with the message: fix(smoke): typo. Change nothing else and run no tests."}]}' >"$TMP/items.json"
mkdir -p "$TMP/cfg"
jq -n --arg m "$MODEL" --arg e "$EFFORT" '{prBabysit: {hosts: ["claude"], jev: false,
  routing: {claude: [range(4) | {model: $m, effort: $e}]}}}' >"$TMP/cfg/toolu.config.json"
TOOLU_CONFIG_DIR="$TMP/cfg" TOOLU_PROJECT_DIR="$TMP/cfg" \
  bash "$S/route-fix.sh" --items "$TMP/items.json" --host claude --no-jev >"$TMP/route.json"
[ "$(jq -r '.dispatch' "$TMP/route.json")" = herdr ] || fail "route did not dispatch to herdr: $(cat "$TMP/route.json")"
[ "$(jq -c '[.groups[] | .tier]' "$TMP/route.json")" = '["standard","trivial"]' ] || fail "expected two tier groups: $(cat "$TMP/route.json")"
step "route: $(jq -c '[.groups[] | {tier, host, model, effort}]' "$TMP/route.json")"

# ---- start: worktree + fixer agent
out=$(bash "$S/dispatch-fix.sh" start --state-file "$STATE" --plan "$TMP/route.json" --items "$TMP/items.json" \
  --repo-root "$CLONE" --branch feat/smoke --base main) || fail "start failed: $out"
step "start: $(jq -c '{status, reason, worktree, branch, agent: .groups[0].agent}' <<<"$out")"
[ "$(jq -r .status <<<"$out")" = running ] || fail "start status is not running: $out"
WT=$(jq -r .worktree <<<"$out")
AGENT=$(jq -r '.groups[0].agent' <<<"$out")
[ -d "$WT" ] || fail "worktree path missing: $WT"
[ "$(git -C "$WT" branch --show-current)" = pr-babysit/local-pb-smoke-1 ] || fail "worktree is not on pr-babysit/local-pb-smoke-1"

# ---- boundary: a second start is refused while the fixer runs
rc=0; again=$(bash "$S/dispatch-fix.sh" start --state-file "$STATE" --plan "$TMP/route.json" --items "$TMP/items.json" \
  --repo-root "$CLONE" --branch feat/smoke --base main) || rc=$?
[ "$rc" -eq 3 ] && [ "$(jq -r '.errors[0].code' <<<"$again")" = fixer_running ] || fail "second start was not refused: rc=$rc $again"
step "second start: fixer_running (exit 3)"

# ---- wait until the fixer settles (bounded)
status=running
for _ in 1 2 3 4 5 6; do
  out=$(bash "$S/dispatch-fix.sh" wait --state-file "$STATE" --timeout-seconds 150)
  status=$(jq -r .status <<<"$out")
  step "wait: $(jq -c '{status, reason, commits}' <<<"$out")"
  [ "$status" = running ] || break
done
[ "$status" = "done" ] || fail "fixer did not finish done: $out"
[ "$(jq -c '[.groups[] | .status]' <<<"$out")" = '["done","done"]' ] || fail "both groups should be done: $out"
[ "$(jq '.commits | length' <<<"$out")" -eq 2 ] || fail "expected exactly two fixer commits: $out"
changed=$(git -C "$WT" log --name-only --format= "refs/remotes/origin/feat/smoke..HEAD" | sort -u | sed '/^$/d')
[ "$changed" = smoke.txt ] || fail "fixer commits touched: $changed"
tail -1 "$WT/smoke.txt" | grep -q 'fixed by pr-babysit smoke' || fail "smoke.txt does not end with the first fixer's line"
[ "$(head -1 "$WT/smoke.txt")" = "hello world" ] || fail "the second fixer did not fix the first line"

# ---- the controller's push (to the local origin), then cleanup
git -C "$WT" push --quiet origin "HEAD:feat/smoke"
out=$(bash "$S/dispatch-fix.sh" cleanup --state-file "$STATE") || fail "cleanup failed: $out"
step "cleanup: $out"
[ "$(jq -r '.worktreeRemoved and .branchDeleted' <<<"$out")" = true ] || fail "cleanup left something behind: $out"
[ ! -d "$WT" ] || fail "worktree directory still exists: $WT"
rmdir "$(dirname "$WT")" 2>/dev/null || true   # herdr's per-repo parent, empty for this throwaway repo
[ "$(git -C "$CLONE" worktree list | wc -l | tr -d ' ')" -eq 1 ] || fail "git still lists a linked worktree"
[ -z "$(git -C "$CLONE" branch --list 'pr-babysit/*')" ] || fail "a pr-babysit/* branch is left"
herdr worktree list --cwd "$CLONE" | jq -e '[.result.worktrees[]? | select((.branch // "") | startswith("pr-babysit/"))] | length == 0' >/dev/null \
  || fail "herdr still lists a pr-babysit worktree"
herdr agent list | jq -e --arg a "$AGENT" '[.result.agents[]? | select(.name == $a)] | length == 0' >/dev/null \
  || fail "fixer agent $AGENT is still live"
[ "$(jq -c '[.herdrWorktree, .fixer]' "$STATE")" = '[null,null]' ] || fail "state still records the worktree or fixer"
