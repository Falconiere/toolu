#!/usr/bin/env bash
# Content-addressed branch-diff hash: the one formula push-review, plan-ledger
# (both the checker lib and the push-time gate), and docs-sync each computed as
# their own inline copy. Survives amend/rebase (content, not commit ids).
#
# Public API:
#   toolu_diff_sha REPO_ROOT BASE_REF  - print the hash of `git diff
#                                        BASE_REF...HEAD` run in REPO_ROOT.
#                                        Non-zero + empty stdout only when git
#                                        fails or prints nothing; an empty
#                                        diff returns 0 with the well-known
#                                        empty-blob sha — callers own the
#                                        sentinel.
#
# NO sentinel mapping here (e.g. push-review's well-known-empty-blob-sha ->
# "empty-diff" substitution) — an empty diff prints the real (empty-blob) sha
# like any other; each caller decides what an empty diff means to it.
#
# Source via:  . "${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}/diff-sha.sh"

# pipefail so a failing `git diff` isn't masked by `git hash-object` succeeding
# on the resulting empty stream (which hashes to the well-known empty-blob sha).
set -o pipefail

# toolu_diff_sha REPO_ROOT BASE_REF
toolu_diff_sha() {
  local repo_root="$1" base_ref="$2" sha
  sha=$(git -C "$repo_root" diff --no-color "${base_ref}...HEAD" 2>/dev/null \
    | git hash-object --stdin 2>/dev/null) || return 1
  [ -n "$sha" ] || return 1
  printf '%s\n' "$sha"
}
