#!/usr/bin/env bash
# Push-review waivers — "yes, push anyway", remembered for exactly that diff.
#
# A PreToolUse hook never learns how the user answered its `ask`. PostToolUse
# does, implicitly: it runs only when the tool actually ran. So the two halves
# talk through a file.
#
#   1. push-review.sh asks, and records a PENDING waiver naming the diff SHA
#      it asked about.
#   2. post-tools/modules/push-waiver.sh sees the push happened, and promotes
#      the pending marker to a real waiver — but only when the push SUCCEEDED
#      and the SHA still matches.
#   3. push-review.sh finds the waiver on the next push of that same diff and
#      stays quiet. A new commit changes the SHA, so the waiver stops applying
#      and the gate asks again.
#
# A refused ask simply leaves the pending marker behind; nothing promotes it,
# and the state sweeper reclaims it. Both writes are atomic (tmp + mv) so a
# half-written waiver can never read as a valid one.
#
# Public API:
#   push_waiver_dir ROOT
#   push_waiver_path ROOT SLUG
#   push_waiver_pending_path ROOT SLUG
#   push_waiver_matches ROOT SLUG SHA
#   push_waiver_pend ROOT SLUG SHA BASE REASON_CODE
#   push_waiver_promote ROOT SLUG SHA

_TOOLU_PUSH_WAIVER_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=host.sh
. "$_TOOLU_PUSH_WAIVER_LIB_DIR/host.sh"

PUSH_WAIVER_VERSION=1

# push_waiver_dir ROOT -> the push-review state directory for ROOT.
# $STATE_DIR overrides, matching push-review.sh and the toolu-review state
# writer, so tests and worktree flows point all three at the same place.
push_waiver_dir() {
  local root="${1:-}"
  printf '%s' "${STATE_DIR:-$(toolu_project_state_dir push-review "$root")}"
}

push_waiver_path() {
  printf '%s/%s.waiver.json' "$(push_waiver_dir "$1")" "$2"
}

push_waiver_pending_path() {
  printf '%s/%s.pending-waiver.json' "$(push_waiver_dir "$1")" "$2"
}

# _push_waiver_write FILE JSON -> atomic write, 0 on success.
# A failed mktemp or write leaves no file at all rather than a partial one;
# the caller decides whether that is fatal (it never is — a missing waiver
# just means the gate asks again).
_push_waiver_write() {
  local file="$1" json="$2" tmp
  mkdir -p "$(dirname "$file")" 2>/dev/null || return 1
  tmp=$(mktemp "${file}.XXXXXX" 2>/dev/null) || return 1
  if printf '%s\n' "$json" > "$tmp" 2>/dev/null; then
    mv -f "$tmp" "$file" 2>/dev/null && return 0
  fi
  rm -f "$tmp" 2>/dev/null
  return 1
}

# _push_waiver_sha FILE -> print the diff_sha of a v1 waiver-shaped FILE.
# Prints nothing (and returns 1) for a missing file, unreadable JSON, or a
# version this code does not understand — all of which must read as "no
# waiver", never as a match.
_push_waiver_sha() {
  local file="$1" version sha
  [ -f "$file" ] || return 1
  version=$(jq -r '.version // ""' "$file" 2>/dev/null) || return 1
  [ "$version" = "$PUSH_WAIVER_VERSION" ] || return 1
  sha=$(jq -r '.diff_sha // ""' "$file" 2>/dev/null) || return 1
  [ -n "$sha" ] || return 1
  printf '%s' "$sha"
}

# push_waiver_matches ROOT SLUG SHA -> 0 iff a waiver covers exactly SHA.
push_waiver_matches() {
  local root="$1" slug="$2" sha="$3" recorded
  [ -n "$sha" ] || return 1
  recorded=$(_push_waiver_sha "$(push_waiver_path "$root" "$slug")") || return 1
  [ "$recorded" = "$sha" ]
}

# push_waiver_pend ROOT SLUG SHA BASE REASON_CODE
# Record that the user was asked about SHA. Overwrites any earlier pending
# marker: only the most recent question is answerable.
push_waiver_pend() {
  local root="$1" slug="$2" sha="$3" base="$4" code="$5" json now
  [ -n "$sha" ] || return 1
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  json=$(jq -cn --argjson version "$PUSH_WAIVER_VERSION" --arg branch "$slug" \
    --arg sha "$sha" --arg base "$base" --arg code "$code" --arg now "$now" \
    '{version: $version, branch: $branch, diff_sha: $sha, base_branch: $base,
      reason_code: $code, asked_at: $now}') || return 1
  _push_waiver_write "$(push_waiver_pending_path "$root" "$slug")" "$json"
}

# push_waiver_promote ROOT SLUG SHA -> 0 iff a waiver was written.
# The SHA guard is the whole point: a pending marker from an older diff must
# not be cashed in by a push of different code.
push_waiver_promote() {
  local root="$1" slug="$2" sha="$3" pending pending_sha json now
  [ -n "$sha" ] || return 1
  pending=$(push_waiver_pending_path "$root" "$slug")
  pending_sha=$(_push_waiver_sha "$pending") || return 1
  [ "$pending_sha" = "$sha" ] || return 1
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  json=$(jq -c --arg now "$now" 'del(.asked_at) + {waived_at: $now}' "$pending" 2>/dev/null) || return 1
  _push_waiver_write "$(push_waiver_path "$root" "$slug")" "$json" || return 1
  rm -f "$pending" 2>/dev/null
  return 0
}
