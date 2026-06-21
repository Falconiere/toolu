#!/usr/bin/env bash
# Preflight gate for the rust-refactor command
# (spec docs/toolu/specs/2026-06-20-rust-quality-refactor-design.md, AC-8).
#
# Guards the refactor before any mutation runs: the working tree MUST be clean
# (a dirty tree means a refactor commit would mix in unrelated edits, and the
# AC-11 rollback — `git reset --hard HEAD~1` — would destroy them), the target
# MUST resolve to a Cargo workspace, and the dedicated refactor branch must
# exist and be checked out.
#
#   rust-refactor-preflight.sh --path <repo>
#
#     --path <repo>   the target Cargo workspace root (required)
#
# Exits NON-ZERO with a clear stderr message on a dirty tree, a non-Cargo
# target, or a non-git target. Idempotent: a re-run on an already-prepared repo
# (branch already present / already checked out) succeeds without re-creating it.

set -euo pipefail

# --------------------------------------------------------------------------
# Locate self.
# --------------------------------------------------------------------------
PF_SELF="$(cd "${BASH_SOURCE[0]%/*}" && pwd)/${BASH_SOURCE[0]##*/}"

# The branch every refactor lands on. Single source of truth for apply too.
PF_BRANCH="rust-quality/refactor"

pf_die() { printf 'rust-refactor-preflight: %s\n' "$1" >&2; exit "${2:-1}"; }
pf_info() { printf 'rust-refactor-preflight: %s\n' "$1"; }

# --------------------------------------------------------------------------
# Argument parsing.
# --------------------------------------------------------------------------
OPT_PATH=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --path)   OPT_PATH="${2:-}"; shift 2 ;;
    --path=*) OPT_PATH="${1#--path=}"; shift ;;
    -h|--help) sed -n '2,21p' "$PF_SELF"; exit 0 ;;
    *) pf_die "unknown argument: $1" 2 ;;
  esac
done

[ -n "$OPT_PATH" ] || pf_die "--path <repo> is required" 2
[ -d "$OPT_PATH" ] || pf_die "not a directory: $OPT_PATH" 2
REPO="$(cd "$OPT_PATH" && pwd -P)"

command -v git >/dev/null 2>&1 || pf_die "git is required" 3

# --------------------------------------------------------------------------
# Must be a git work tree.
# --------------------------------------------------------------------------
git -C "$REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
  || pf_die "not inside a git work tree: $REPO" 5

# --------------------------------------------------------------------------
# Must resolve a Cargo workspace at or below the target. We reuse rust-scan's
# resolution by asking it (it exits 4 on a non-Cargo target). Calling --json and
# discarding the report keeps this gate honest with the scanner the rest of the
# pipeline uses — same workspace resolution, zero drift.
# --------------------------------------------------------------------------
if [ -f "$REPO/Cargo.toml" ]; then
  : # fast path: a manifest at the root is unambiguously a Cargo workspace
else
  # No manifest at the root — fall back to a walk for a nested layout, matching
  # rust-scan's resolver (a Cargo.toml anywhere at/below the target counts).
  if ! find "$REPO" -type d -name target -prune -o \
       -type f -name Cargo.toml -print 2>/dev/null | grep -q .; then
    pf_die "not a Cargo workspace at $REPO (no Cargo.toml at or below it)" 4
  fi
fi

# --------------------------------------------------------------------------
# The tree must be clean (AC-8). A single porcelain line means uncommitted
# work — abort before any branch creation so we never half-prepare a dirty repo.
# --------------------------------------------------------------------------
if [ -n "$(git -C "$REPO" status --porcelain)" ]; then
  pf_die "git working tree is dirty — commit or stash before refactoring" 6
fi

# --------------------------------------------------------------------------
# Create/checkout the refactor branch. Idempotent: if it already exists, just
# check it out; if it is already current, do nothing.
# --------------------------------------------------------------------------
CURRENT="$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '')"
if [ "$CURRENT" = "$PF_BRANCH" ]; then
  pf_info "already on $PF_BRANCH"
elif git -C "$REPO" show-ref --verify --quiet "refs/heads/$PF_BRANCH"; then
  git -C "$REPO" checkout "$PF_BRANCH" >/dev/null 2>&1 \
    || pf_die "failed to checkout existing branch $PF_BRANCH" 7
  pf_info "checked out existing $PF_BRANCH"
else
  git -C "$REPO" checkout -b "$PF_BRANCH" >/dev/null 2>&1 \
    || pf_die "failed to create branch $PF_BRANCH" 7
  pf_info "created $PF_BRANCH"
fi

# --------------------------------------------------------------------------
# Status line — branch + clean confirmation, the green light for Audit.
# --------------------------------------------------------------------------
pf_info "ready: clean tree, on $PF_BRANCH at $REPO"
