#!/usr/bin/env bash
# write-state.sh — write the push-review state file the toolu push-review
# gate validates (.claude/tmp/push-review/<branch-slug>.json).
#
# The review JUDGMENT (which findings exist) belongs to the caller — the
# `code-review:review` skill. This script does only the deterministic
# bookkeeping: compute the gate's diff_sha/base/slug, bump review_round, and
# write the JSON atomically.
#
# The base / diff_sha / slug recipes are MIRRORS of the gate in
# plugins/toolu/hooks/pre-tools/modules/push-review.sh — the cross-check in
# scripts/__tests__/state-writer.bats asserts they produce identical SHAs, so a
# drift in either recipe fails CI. Harmless no-op when the toolu gate is not
# installed (the file is simply never read).
#
# Usage: write-state.sh --findings-count N [--reviewers JSON] [--findings JSON]
#   --findings-count  (required) integer; the gate allows push only when 0.
#   --reviewers       (default ["code-review:review"]) JSON array.
#   --findings        (default []) JSON array of {path,severity,text}.
# Prints the state file path on success.
set -o pipefail

findings_count=""
reviewers='["code-review:review"]'
findings='[]'
while [ $# -gt 0 ]; do
  case "$1" in
    --findings-count) findings_count="$2"; shift 2 ;;
    --reviewers)      reviewers="$2";      shift 2 ;;
    --findings)       findings="$2";       shift 2 ;;
    *) echo "write-state.sh: unknown arg: $1" >&2; exit 2 ;;
  esac
done

[ -n "$findings_count" ] || { echo "write-state.sh: --findings-count required" >&2; exit 2; }
[[ "$findings_count" =~ ^[0-9]+$ ]] || { echo "write-state.sh: --findings-count must be an integer" >&2; exit 2; }
command -v jq  >/dev/null 2>&1 || { echo "write-state.sh: jq required"  >&2; exit 2; }
command -v git >/dev/null 2>&1 || { echo "write-state.sh: git required" >&2; exit 2; }

branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
{ [ -n "$branch" ] && [ "$branch" != "HEAD" ]; } || { echo "write-state.sh: not on a branch (detached HEAD?)" >&2; exit 1; }

# Base branch — mirror of detect_base_branch's core; $PUSH_REVIEW_BASE matches
# the gate's own override.
base="${PUSH_REVIEW_BASE:-}"
if [ -z "$base" ]; then
  base=$(git symbolic-ref --quiet refs/remotes/origin/HEAD 2>/dev/null | sed 's#refs/remotes/origin/##')
  [ -n "$base" ] || base="main"
fi

# diff_sha — MIRROR of push-review.sh:95 (cross-checked by state-writer.bats).
diff_sha=$(git diff --no-color "${base}...HEAD" 2>/dev/null | git hash-object --stdin 2>/dev/null || echo "")
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

# slug — MIRROR of push-review.sh:_branch_slug.
slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
[ -n "$slug" ] || slug="_default"

state_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}/.claude/tmp/push-review"
state_file="$state_dir/${slug}.json"
mkdir -p "$state_dir" || { echo "write-state.sh: cannot create $state_dir" >&2; exit 1; }

# review_round: read existing // 0, then +1 (each rewrite bumps; gate caps it).
prev_round=0
if [ -f "$state_file" ]; then
  prev_round=$(jq -r '.review_round // 0' "$state_file" 2>/dev/null || echo 0)
  [[ "$prev_round" =~ ^[0-9]+$ ]] || prev_round=0
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
  '{version:2, branch:$branch, diff_sha:$diff_sha, base_branch:$base,
    reviewed_at:$reviewed_at, reviewers:$reviewers,
    findings_count:$findings_count, findings:$findings, review_round:$review_round}' \
  > "$tmp"; then
  rm -f "$tmp"; echo "write-state.sh: jq failed (bad --reviewers/--findings JSON?)" >&2; exit 1
fi
mv "$tmp" "$state_file" || { rm -f "$tmp"; echo "write-state.sh: atomic mv failed" >&2; exit 1; }
echo "$state_file"
