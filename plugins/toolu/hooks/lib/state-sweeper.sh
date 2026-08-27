#!/usr/bin/env bash
# Transient-state sweeper for <repo>/<host-dir>/tmp.
#
# toolu's gates key their state by branch, and nothing ever reclaimed it: a
# push-review state for a branch merged three weeks ago, a plan ledger for a
# branch that no longer exists, a quality-gate record naming files that were
# deleted. The pile is harmless right up until it is confusing, and the fix
# was `rm -rf .claude/tmp/*` by hand.
#
# This runs once at SessionStart and reclaims only what is provably spent:
#
#   push-review/, plan-ledger/, docs-sync/  per-branch state — dropped when the
#     branch is gone, merged into base, or older than the TTL. The CURRENT
#     branch is never touched, however old: it is the one you are working on.
#   quality-gate-status.json  dropped when passing, or when failing about files
#     that no longer exist. A failing record about live files SURVIVES — a
#     violation is not fixed by forgetting it.
#   telemetry/<slug>.jsonl  trimmed to the retention window, never deleted
#     wholesale; the history is the point.
#
# Anything else under tmp/ — including the permissions sentinel — is not ours
# and is left alone.
#
# Everything here is best-effort: a failure warns and returns 0, because a
# session must start whether or not old scratch files could be cleaned.
#
# Public API:
#   toolu_sweep_state ROOT

_TOOLU_SWEEPER_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=config.sh
. "$_TOOLU_SWEEPER_LIB_DIR/config.sh"
# shellcheck source=detect.sh
. "$_TOOLU_SWEEPER_LIB_DIR/detect.sh"

TOOLU_SWEEP_DEFAULT_TTL_HOURS=24
TOOLU_SWEEP_DEFAULT_RETENTION_DAYS=7
TOOLU_SWEEP_BRANCH_DIRS="push-review plan-ledger docs-sync"

_toolu_sweep_warn() {
  printf 'toolu-sweep: %s\n' "$1" >&2
}

# _toolu_sweep_number PATH DEFAULT -> a positive integer from config, or DEFAULT.
# A zero, a negative, or a non-number means "not configured" rather than an
# error — the same lenient reading the quality thresholds use.
_toolu_sweep_number() {
  local path="$1" def="$2" val
  toolu_load_config
  if [ "$_TOOLU_HAS_JQ" != "1" ]; then
    printf '%s' "$def"
    return 0
  fi
  val=$(jq -r --arg p "$path" '
    ($p | split(".")) as $ks
    | (getpath($ks)) as $v
    | if ($v | type) == "number" and $v > 0 then ($v | floor | tostring) else "" end
  ' <<< "$TOOLU_CFG_JSON" 2>/dev/null)
  [ -n "$val" ] && printf '%s' "$val" || printf '%s' "$def"
}

# _toolu_sweep_cutoff DAYS -> an ISO-8601 UTC timestamp DAYS ago.
# GNU date first, BSD/macOS second; prints nothing if neither works, which
# callers read as "cannot judge age, keep everything".
_toolu_sweep_cutoff() {
  local days="$1"
  date -u -d "-${days} days" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
    || date -u -v-"${days}"d +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
    || return 1
}

# _toolu_sweep_slug_of FILE -> the branch slug a state file belongs to.
# Strips the compound suffixes first so feat_x.waiver.json and
# feat_x.pending-waiver.json both resolve to feat_x.
_toolu_sweep_slug_of() {
  local base
  base=$(basename "$1")
  base="${base%.json}"
  base="${base%.pending-waiver}"
  base="${base%.waiver}"
  printf '%s' "$base"
}

# _toolu_sweep_reclaimable SLUG LIVE MERGED FILE TTL -> 0 iff FILE can go.
_toolu_sweep_reclaimable() {
  local slug="$1" live="$2" merged="$3" file="$4" ttl_hours="$5"
  case "$merged" in *" $slug "*) return 0 ;; esac
  case "$live" in *" $slug "*) ;; *) return 0 ;; esac
  # Branch still exists and is unmerged: age is the only remaining reason.
  [ -n "$(find "$file" -maxdepth 0 -mmin "+$((ttl_hours * 60))" 2>/dev/null)" ]
}

# _toolu_sweep_branch_state ROOT STATE_ROOT TTL -> prune per-branch state dirs.
_toolu_sweep_branch_state() {
  local root="$1" state_root="$2" ttl_hours="$3"
  local base_branch current_branch current_slug live=" " merged=" " name dir file slug

  current_branch=$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  current_slug=$(branch_slug "$current_branch")
  base_branch=$(detect_base_branch "$root")

  while IFS= read -r name; do
    [ -n "$name" ] && live="${live}$(branch_slug "$name") "
  done < <(git -C "$root" branch --format='%(refname:short)' 2>/dev/null)

  # `--merged` sees ancestry only, so a SQUASH-merged branch never appears here.
  # Those are reclaimed by the TTL path instead — which is exactly why the TTL
  # exists alongside this check rather than being redundant with it.
  while IFS= read -r name; do
    [ -n "$name" ] && merged="${merged}$(branch_slug "$name") "
  done < <(git -C "$root" branch --merged "$base_branch" --format='%(refname:short)' 2>/dev/null)

  for dir in $TOOLU_SWEEP_BRANCH_DIRS; do
    [ -d "$state_root/$dir" ] || continue
    for file in "$state_root/$dir"/*.json; do
      [ -f "$file" ] || continue
      slug=$(_toolu_sweep_slug_of "$file")
      [ "$slug" = "$current_slug" ] && continue
      if _toolu_sweep_reclaimable "$slug" "$live" "$merged" "$file" "$ttl_hours"; then
        rm -f "$file" 2>/dev/null || _toolu_sweep_warn "could not remove $file"
      fi
    done
  done
}

# _toolu_sweep_gate_file GATE_FILE -> drop a spent quality-gate record.
_toolu_sweep_gate_file() {
  local gate_file="$1" status live_violation entry
  [ -f "$gate_file" ] || return 0
  status=$(jq -r '.status // ""' "$gate_file" 2>/dev/null) || return 0
  if [ "$status" = "passing" ]; then
    rm -f "$gate_file" 2>/dev/null || _toolu_sweep_warn "could not remove $gate_file"
    return 0
  fi
  [ "$status" = "failing" ] || return 0

  # A failing record is only spent when every file it complains about is gone.
  # Age alone must never clear it: that would silently reopen a real violation.
  live_violation=0
  while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    [ "$entry" = "__global__" ] && { live_violation=1; break; }
    [ -e "$entry" ] && { live_violation=1; break; }
  done < <(jq -r 'if (.entries? | type) == "object" then (.entries | keys[]) else (.file // "__global__") end' \
             "$gate_file" 2>/dev/null)

  [ "$live_violation" = "1" ] && return 0
  rm -f "$gate_file" 2>/dev/null || _toolu_sweep_warn "could not remove $gate_file"
}

# _toolu_sweep_telemetry DIR DAYS -> trim each jsonl to the retention window.
_toolu_sweep_telemetry() {
  local dir="$1" days="$2" cutoff file tmp
  [ -d "$dir" ] || return 0
  cutoff=$(_toolu_sweep_cutoff "$days") || return 0
  for file in "$dir"/*.jsonl; do
    [ -f "$file" ] || continue
    tmp=$(mktemp "${file}.XXXXXX" 2>/dev/null) || continue
    # A file jq cannot parse is left exactly as it is: it may be another
    # tool's, or a partially written line worth keeping for debugging.
    if jq -c --arg cutoff "$cutoff" 'select((.t // "") >= $cutoff)' "$file" > "$tmp" 2>/dev/null; then
      if [ -s "$tmp" ]; then
        mv -f "$tmp" "$file" 2>/dev/null || rm -f "$tmp" 2>/dev/null
      else
        rm -f "$tmp" "$file" 2>/dev/null
      fi
    else
      rm -f "$tmp" 2>/dev/null
    fi
  done
}

# toolu_sweep_state ROOT -> reclaim spent transient state. Always returns 0.
toolu_sweep_state() {
  local root="${1:-}" state_root ttl_hours retention_days
  [ -n "$root" ] || root=$(toolu_project_root)
  [ -n "$root" ] || return 0
  command -v jq >/dev/null 2>&1 || return 0

  # Opt-out, and a hard requirement of git for the branch questions below.
  toolu_enabled gates sweep || return 0
  command -v git >/dev/null 2>&1 || return 0

  state_root=$(toolu_project_state_root "$root")
  [ -n "$state_root" ] && [ -d "$state_root" ] || return 0

  ttl_hours=$(_toolu_sweep_number gates.stateTtlHours "$TOOLU_SWEEP_DEFAULT_TTL_HOURS")
  retention_days=$(_toolu_sweep_number gates.telemetryRetentionDays "$TOOLU_SWEEP_DEFAULT_RETENTION_DAYS")

  _toolu_sweep_branch_state "$root" "$state_root" "$ttl_hours"
  _toolu_sweep_gate_file "$state_root/quality-gate-status.json"
  _toolu_sweep_telemetry "$state_root/telemetry" "$retention_days"
  return 0
}
