#!/usr/bin/env bash
# write-state.sh — write the push-review state file the toolu push-review
# gate validates (.claude/tmp/push-review/<branch-slug>.json).
#
# The review JUDGMENT (which findings exist) belongs to the caller — the
# `toolu-review:review` skill. This script does only the deterministic
# bookkeeping: compute the gate's diff_sha/base/slug, bump review_round, and
# write the JSON atomically.
#
# The base / diff_sha / slug / reviewed_files recipes are MIRRORS of the gate
# in plugins/toolu/hooks/pre-tools/modules/push-review.sh — the cross-check in
# scripts/__tests__/state-writer.bats asserts they produce identical SHAs (and,
# for reviewed_files, an identical file list), so a drift in either recipe
# fails CI. This mirror is deliberate (cross-plugin sourcing is barred here —
# see spec Non-Goal 7): keep it local rather than sourcing toolu's diff-sha.sh.
# Harmless no-op when the toolu gate is not installed (the file is simply
# never read).
#
# Usage: write-state.sh --findings-count N [--reviewers JSON] [--findings JSON]
#                       [--repo PATH] [--reviewed-files a.ts,b.rs]
#   --findings-count  (required) integer; the gate allows push only when 0.
#   --reviewers       (default ["toolu-review:review"]) JSON array.
#   --findings        (default []) JSON array of {path,severity,text}.
#   --repo            (default: the cwd's repo root) the checkout being reviewed.
#                     Pass the worktree path when reviewing a worktree from a
#                     session rooted elsewhere — the gate reads the state file
#                     under the pushed repo's own root, so writing it anywhere
#                     else is invisible to the gate.
#   --reviewed-files  (default: auto-computed) comma-separated list of paths,
#                     overriding the auto-computed `git diff --name-only
#                     <base>...HEAD` file list. The gate (schema v2) requires
#                     `reviewed_files` (sorted, unique) to equal the diff's
#                     changed paths exactly, so only override this when the
#                     review genuinely covered a different path set than the
#                     one auto-detected here.
# Prints the state file path on success.
set -o pipefail

findings_count=""
reviewers='["toolu-review:review"]'
findings='[]'
repo=""
reviewed_files_arg=""
reviewed_files_set=0
while [ $# -gt 0 ]; do
  case "$1" in
    --findings-count) findings_count="$2"; shift 2 ;;
    --reviewers)      reviewers="$2";      shift 2 ;;
    --findings)       findings="$2";       shift 2 ;;
    --repo)           repo="$2";           shift 2 ;;
    --reviewed-files) reviewed_files_arg="$2"; reviewed_files_set=1; shift 2 ;;
    *) echo "write-state.sh: unknown arg: $1" >&2; exit 2 ;;
  esac
done

[ -n "$findings_count" ] || { echo "write-state.sh: --findings-count required" >&2; exit 2; }
[[ "$findings_count" =~ ^[0-9]+$ ]] || { echo "write-state.sh: --findings-count must be an integer" >&2; exit 2; }
command -v jq  >/dev/null 2>&1 || { echo "write-state.sh: jq required"  >&2; exit 2; }
command -v git >/dev/null 2>&1 || { echo "write-state.sh: git required" >&2; exit 2; }

# Repo root — MIRROR of push_target_root's fallback chain. The gate resolves
# the root from the push command (`git -C <path> push`), so the writer must be
# able to name the same checkout or the two disagree in a worktree.
repo_root=$(git -C "${repo:-.}" rev-parse --show-toplevel 2>/dev/null || echo "")
[ -n "$repo_root" ] || { echo "write-state.sh: ${repo:-$(pwd)} is not inside a git repo" >&2; exit 1; }

branch=$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
{ [ -n "$branch" ] && [ "$branch" != "HEAD" ]; } || { echo "write-state.sh: not on a branch (detached HEAD?)" >&2; exit 1; }

# Base branch — mirror of detect_base_branch's core; $PUSH_REVIEW_BASE matches
# the gate's own override.
base="${PUSH_REVIEW_BASE:-}"
if [ -z "$base" ]; then
  base=$(git -C "$repo_root" symbolic-ref --quiet refs/remotes/origin/HEAD 2>/dev/null | sed 's#refs/remotes/origin/##')
  [ -n "$base" ] || base="main"
fi

# diff_sha — MIRROR of push-review.sh's current_diff_sha (cross-checked by
# state-writer.bats).
diff_sha=$(git -C "$repo_root" diff --no-color "${base}...HEAD" 2>/dev/null | git -C "$repo_root" hash-object --stdin 2>/dev/null || echo "")
[ -n "$diff_sha" ] || { echo "write-state.sh: git diff ${base}...HEAD failed" >&2; exit 1; }

# Refuse the empty-blob SHA: an empty diff against base means nothing diverged
# (or a force-reset). The push-review gate treats this sentinel as never-matching
# and denies, so writing a state file with it just yields a confusing
# "review recorded" → "diff is empty" deny. Fail early with an actionable message.
EMPTY_BLOB_SHA="e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
if [ "$diff_sha" = "$EMPTY_BLOB_SHA" ]; then
  echo "write-state.sh: diff against ${base} is empty; nothing to review yet" >&2
  exit 1
fi

# reviewed_files — MIRROR of the gate's `changed_sorted` (push-review.sh):
# auto-computed from the real diff's changed paths (sorted, unique) unless
# --reviewed-files overrides it. The gate denies unless this list equals its
# own `git diff --name-only` computation exactly, so both branches emit a
# sorted-unique JSON array of strings.
if [ "$reviewed_files_set" -eq 1 ]; then
  reviewed_files_json=$(printf '%s' "$reviewed_files_arg" \
    | jq -R 'split(",") | map(select(length > 0)) | unique')
else
  reviewed_files_json=$(git -C "$repo_root" diff --no-color "${base}...HEAD" --name-only 2>/dev/null \
    | sort -u | jq -R -s 'split("\n") | map(select(length > 0))')
fi
# jq always prints at least "[]" on success, so empty stdout here only
# happens on a real git/jq failure — surface it rather than writing a state
# file with a silently-wrong (and gate-denying) reviewed_files.
[ -n "$reviewed_files_json" ] || { echo "write-state.sh: failed to compute reviewed_files" >&2; exit 1; }

# slug — MIRROR of push-review.sh:_branch_slug.
slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
[ -n "$slug" ] || slug="_default"

# state_dir — MIRROR of the gate's resolution, including its $STATE_DIR override.
state_dir="${STATE_DIR:-$repo_root/.claude/tmp/push-review}"
state_file="$state_dir/${slug}.json"
mkdir -p "$state_dir" || { echo "write-state.sh: cannot create $state_dir" >&2; exit 1; }

# review_round counts rewrites AT THE SAME diff_sha, so the gate's MAX_ROUNDS
# cap means "reviewers keep finding issues in code that never changed". A
# changed diff restarts at 1: the previous rounds judged different code, and
# nothing ever resets the counter after a push, so carrying it forward made the
# cap terminal for any branch that lived long enough.
prev_round=0
if [ -f "$state_file" ]; then
  prev_sha=$(jq -r '.diff_sha // ""' "$state_file" 2>/dev/null || echo "")
  if [ "$prev_sha" = "$diff_sha" ]; then
    prev_round=$(jq -r '.review_round // 0' "$state_file" 2>/dev/null || echo 0)
    [[ "$prev_round" =~ ^[0-9]+$ ]] || prev_round=0
  fi
fi
review_round=$((prev_round + 1))

reviewed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)

tmp="$state_file.tmp.$$"
if ! jq -n \
  --arg branch "$branch" \
  --arg diff_sha "$diff_sha" \
  --arg base "$base" \
  --arg reviewed_at "$reviewed_at" \
  --argjson reviewers "$reviewers" \
  --argjson findings_count "$findings_count" \
  --argjson findings "$findings" \
  --argjson review_round "$review_round" \
  --argjson reviewed_files "$reviewed_files_json" \
  '{version:2, branch:$branch, diff_sha:$diff_sha, base_branch:$base,
    reviewed_at:$reviewed_at, reviewers:$reviewers,
    findings_count:$findings_count, findings:$findings, review_round:$review_round,
    reviewed_files:$reviewed_files}' \
  > "$tmp"; then
  rm -f "$tmp"; echo "write-state.sh: jq failed (bad --reviewers/--findings/--reviewed-files JSON?)" >&2; exit 1
fi
mv "$tmp" "$state_file" || { rm -f "$tmp"; echo "write-state.sh: atomic mv failed" >&2; exit 1; }
echo "$state_file"
