#!/usr/bin/env bash
# Stop hook — prompt to save session learnings before exiting.

HOOK_DIR="$(dirname "$0")"

# shellcheck source=lib/detect.sh
. "$HOOK_DIR/lib/detect.sh"
# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

# ── Autonomous comemory maintenance (zero Claude tokens) ───────────────────
# Keeps the retrieval store sharp: mine/prune/gc are all local (no LLM, no API).
# Throttled to once per UTC day via a stamp file; the stamp is written BEFORE
# the work so a mid-run crash never retry-loops the same day. The work is
# detached (backgrounded + disowned) so a hung comemory binary can never delay
# the Stop event on ANY platform — including stock macOS, which ships neither
# `timeout` nor `gtimeout`. When a timeout binary IS present we additionally
# bound the detached run's lifetime; without one the run is unbounded but still
# cannot block Stop, and the once-per-day throttle caps the orphan to one.
#
# Gated on `toolu_enabled hooks session-end` (opt-OUT, default on): unlike
# the reminder below — which is opt-IN — maintenance runs by default, but
# `hooks.session-end: false` disables BOTH, honoring the docs/config.md contract
# that a disabled hook "exits early and emits nothing" (and here, mutates nothing).
if toolu_enabled hooks session-end && [ "$(toolu_comemory_state)" = "available" ]; then
  # Stamp lives in the toolu config dir, NOT comemory's data dir: we don't
  # assume where comemory stores data (the throttle only needs a stable, writable
  # path of our own). $CLAUDE_CONFIG_DIR override is honored for parity with the
  # rest of toolu.
  _cm_stamp_dir="$(toolu_config_root)/toolu"
  _cm_stamp="$_cm_stamp_dir/.comemory-last-maintain"
  _cm_today="$(date -u +%Y%m%d 2>/dev/null || echo '')"
  if [ -n "$_cm_today" ] && [ "$(cat "$_cm_stamp" 2>/dev/null || echo '')" != "$_cm_today" ]; then
    mkdir -p "$_cm_stamp_dir" 2>/dev/null || true
    printf '%s' "$_cm_today" > "$_cm_stamp" 2>/dev/null || true
    # Bound each call's lifetime with timeout/gtimeout when available (absent on
    # stock macOS); the trailing `&` + `disown` detaches the whole sequence so
    # Stop never waits on it regardless.
    _cm_to=""
    if command -v timeout >/dev/null 2>&1; then _cm_to="timeout 30"
    elif command -v gtimeout >/dev/null 2>&1; then _cm_to="gtimeout 30"; fi
    { $_cm_to comemory mine --apply; $_cm_to comemory prune --apply; $_cm_to comemory gc; } >/dev/null 2>&1 &
    disown 2>/dev/null || true
  fi
fi

# OPT-IN: the end-of-session comemory reminder is OFF by default. The agent-memory
# protocol is always-active and saves proactively during the session, so a
# Stop-time nag is redundant noise for most users. It emits only when explicitly
# enabled with `hooks.session-end: true` in toolu.config.json. (This is the
# one hook with opt-in rather than opt-out semantics — see toolu_enabled_explicit.)
if ! toolu_enabled_explicit hooks session-end; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

# Consume stdin (Claude Code sends hook input via stdin)
cat > /dev/null 2>&1 || true

case "$(toolu_comemory_state)" in
  available)
    save_doc="$HOOK_DIR/docs/vector-helper-save.md"
    # The wrapper ships in the comemory plugin; its install path differs per
    # machine, so reference the wrapper name, not a path.
    mod_sh="the comemory plugin's comemory.sh"
    save_hint=$(cat "$save_doc" 2>/dev/null || echo "Save reusable learnings via $mod_sh save.")
    ctx="Session ending. $save_hint"
    ;;
  missing)
    ctx="Session ending. WARN: comemory CLI not installed — session save skipped. Install comemory to persist learnings across sessions."
    ;;
  disabled|*)
    ctx="Session ending."
    ;;
esac

# Stop hooks have no recognized `stopReason` output field; use
# `systemMessage` (valid for every hook event, shown to the user).
# Do NOT use decision:"block" — that would force an extra model turn
# on every Stop.
jq -n --arg ctx "$ctx" '{
  "systemMessage": $ctx
}'

exit 0
