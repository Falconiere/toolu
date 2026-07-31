#!/usr/bin/env bash
# Workflow telemetry append engine: the one write path every gate/lib event
# funnels through, so telemetry.enabled / jq-absent / no-branch all degrade
# the same way regardless of caller.
#
# Public API:
#   telemetry_append REPO_ROOT EVENT [EXTRA_JSON_OBJECT]
#     Appends one line `{"v":1,"t":<iso8601 UTC>,"branch":<branch>,
#     "event":<name>, ...EXTRA_JSON_OBJECT}` to
#     REPO_ROOT/.claude/tmp/telemetry/<branch_slug>.jsonl
#     ($TELEMETRY_DIR overrides the directory). EXTRA_JSON_OBJECT defaults to
#     "{}"; its keys are merged in but can never override v/t/branch/event.
#     Always returns 0 — a telemetry bug must never fail the caller's real
#     work — no-op (nothing written) when:
#       - telemetry.enabled=false in the merged toolu config
#       - jq is missing
#       - HEAD is unborn/detached (mirrors push-review.sh's and docs-sync.sh's
#         own detached-HEAD refusal: branch_slug would otherwise collapse
#         every such caller onto the shared "_default" slug, mixing unrelated
#         branches' events into one file)
#     A malformed EXTRA_JSON_OBJECT is a soft failure: warns on stderr,
#     appends nothing, still returns 0.
#
# CONCURRENCY: exactly one `printf '%s\n' >>` per call — a single O_APPEND
# write. That is atomic only while the line stays under the platform's
# atomic-pipe-write floor (historically 4 KB on POSIX); callers must keep
# EXTRA_JSON_OBJECT small (mirrors gate-file.sh's single-writer assumption).
#
# Source via:  . "${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}/telemetry.sh"

_TELEMETRY_LIB_DIR="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}"
# shellcheck source=./detect.sh
[ -f "$_TELEMETRY_LIB_DIR/detect.sh" ] && . "$_TELEMETRY_LIB_DIR/detect.sh"
# shellcheck source=./config.sh
[ -f "$_TELEMETRY_LIB_DIR/config.sh" ] && . "$_TELEMETRY_LIB_DIR/config.sh"

# telemetry_append REPO_ROOT EVENT [EXTRA_JSON_OBJECT]
telemetry_append() {
  local repo_root="$1" event="$2" extra="${3:-}"
  [ -n "$extra" ] || extra='{}'
  [ -n "$repo_root" ] && [ -n "$event" ] || return 0

  command -v jq  >/dev/null 2>&1 || return 0
  command -v git >/dev/null 2>&1 || return 0

  # Default-on: absent/malformed config -> enabled; explicit false -> no-op.
  if command -v toolu_enabled >/dev/null 2>&1; then
    toolu_enabled telemetry enabled || return 0
  fi

  local branch
  branch=$(git -C "$repo_root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  [[ -z "$branch" || "$branch" == "HEAD" ]] && return 0

  # branch_slug comes from detect.sh, sourced softly above — an older/absent
  # detect.sh must not turn into an unbound-function crash for every caller.
  command -v branch_slug >/dev/null 2>&1 || return 0

  local slug now line dir file
  slug=$(branch_slug "$branch")
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)

  # $extra merged FIRST so the protocol fields (v/t/branch/event) always win
  # on key collision — a caller-supplied "branch" or "event" in extra can
  # never spoof the real ones. A malformed or non-object $extra makes this
  # jq call fail; treat that as the one "malformed extra JSON" soft-failure.
  if ! line=$(jq -cn --argjson extra "$extra" --arg t "$now" --arg branch "$branch" \
        --arg event "$event" '$extra + {v: 1, t: $t, branch: $branch, event: $event}' \
        2>/dev/null) || [ -z "$line" ]; then
    printf 'telemetry: malformed extra JSON for event "%s"; skipping append\n' "$event" >&2
    return 0
  fi

  dir="${TELEMETRY_DIR:-$repo_root/.claude/tmp/telemetry}"
  file="$dir/${slug}.jsonl"
  mkdir -p "$dir" 2>/dev/null || return 0

  printf '%s\n' "$line" >> "$file" 2>/dev/null

  return 0
}
