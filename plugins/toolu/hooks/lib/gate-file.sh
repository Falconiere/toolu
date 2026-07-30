#!/usr/bin/env bash
# Multi-slot writer for .claude/tmp/quality-gate-status.json.
#
# The gate file used to be single-slot (last writer wins): a TS failure
# followed by a Rust failure overwrote the TS record, and clearing the Rust
# file then re-opened the gate with the TS violation still live. The same
# clobber happened between two files of the SAME language.
#
# Now every failing file owns an entry under `entries` (keyed by file path).
# Top-level fields mirror the most recent failure and aggregate all violations,
# so readers (pre-tool gate, statusline, session-start, user-prompt-submit)
# keep their existing .status / .reason / .violations contract untouched.
#
# A legacy or foreign single-slot failing record (e.g. gate-status-hook's
# command failure) is preserved by seeding it into `entries` under its `file`,
# or "__global__" when it has none. Both jq programs below start with the same
# seed step — keep them in sync.
#
# CONCURRENCY: single-writer by assumption. Both functions read-merge-write via
# `mktemp` + `mv -f`, which has a TOCTOU window — two writers that read the same
# `existing` blob would each rewrite it and the last `mv` would drop the other's
# entry. This is safe today because PostToolUse hooks fire serially per tool
# call (one writer at a time). If a parallel-edit flow is ever added, guard both
# functions — AND the single-slot fallback write below, AND the separate
# single-slot passing write in post-tools/modules/gate-status.sh (which writes
# the same gate file) — with `flock` against a `${gate_file}.lock` sentinel; all
# of those paths race identically.
#
# Public API:
#   gate_record_failure GATE_FILE FILE SOURCE REASON VIOLATIONS
#   gate_clear_file     GATE_FILE FILE SOURCE
#
# Telemetry (gate_fail / gate_clear): this file has no repo root of its own to
# hand telemetry_append, so it derives one from GATE_FILE's own path — every
# caller passes a path shaped <root>/.claude/tmp/quality-gate-status.json, so
# three dirname hops off GATE_FILE land back on <root>. telemetry.sh is
# sourced LAZILY (only on first actual emit, not at file load) and every emit
# is guarded by `command -v telemetry_append`, so an absent or older
# telemetry.sh is never a hard dependency for this file's callers.

# _gate_file_root GATE_FILE -> print <root> (three dirname hops off GATE_FILE).
_gate_file_root() {
  dirname "$(dirname "$(dirname "$1")")"
}

# _gate_file_ensure_telemetry -> 0 iff telemetry_append is callable (already
# loaded, or successfully lazy-sourced from telemetry.sh next to this file).
_gate_file_ensure_telemetry() {
  command -v telemetry_append >/dev/null 2>&1 && return 0
  local lib_dir="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}"
  [ -f "$lib_dir/telemetry.sh" ] || return 1
  # shellcheck source=./telemetry.sh
  . "$lib_dir/telemetry.sh"
  command -v telemetry_append >/dev/null 2>&1
}

# gate_record_failure GATE_FILE FILE SOURCE REASON VIOLATIONS
# Adds/replaces this file's entry and marks the gate failing. Writes via a
# temp file so a jq failure can never truncate the gate to an unreadable
# (and therefore silently passing) state; on any error it falls back to the
# legacy single-slot write rather than dropping the failure.
gate_record_failure() {
  local gate_file="$1" file="$2" source="$3" reason="$4" violations="$5"
  local existing='{}' now tmp
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  if [ -f "$gate_file" ]; then
    existing=$(cat "$gate_file" 2>/dev/null) || existing='{}'
    jq -e . <<< "$existing" >/dev/null 2>&1 || existing='{}'
  fi
  tmp=$(mktemp "${gate_file}.XXXXXX" 2>/dev/null) || tmp=""
  if [ -n "$tmp" ] && jq \
      --arg file "$file" --arg source "$source" --arg reason "$reason" \
      --arg violations "$violations" --arg now "$now" '
    (if (.entries? | type) == "object" then .entries
     elif (.status // "") == "failing" then
       { (.file // "__global__"): {
           source: (.source // ""), reason: (.reason // ""),
           violations: (.violations // ""), updatedAt: (.updatedAt // $now) } }
     else {} end) as $prev
    | ($prev + { ($file): {source: $source, reason: $reason,
                           violations: $violations, updatedAt: $now} }) as $entries
    # `violations` aggregates every open entry, intentionally: readers (statusline,
    # session-start, user-prompt-submit) must surface ALL unresolved violations,
    # not just the latest. Growth is bounded by the count of distinct failing
    # files in a session (each clears on the next passing edit), so it stays
    # small in practice. If a long session ever shows this string bloating
    # hot-path reads, add a per-entry cap here rather than dropping entries.
    # sort_by updatedAt then key: the timestamp is second-resolution (BSD `date`
    # on macOS has no %N sub-second granularity), so two failures in the SAME
    # second tiebreak by filename — the top-level .file/.reason mirror may then
    # not be the truly-latest. Cosmetic and self-correcting on the next edit;
    # readers see every failure via `entries` regardless. Accepted over a
    # non-portable %N or a cross-call monotonic counter.
    | { status: "failing", reason: $reason, source: $source, file: $file,
        violations: ([$entries | to_entries | sort_by(.value.updatedAt // "", .key)[]
                      | (.value.violations // "")] | join("")),
        entries: $entries, updatedAt: $now }
  ' <<< "$existing" > "$tmp" 2>/dev/null; then
    mv -f "$tmp" "$gate_file"
  else
    [ -n "$tmp" ] && rm -f "$tmp"
    # Fallback (mktemp or the merge jq failed): write a single-slot record so
    # THIS failure is never lost. That collapses any pre-existing multi-slot
    # `entries` to one record — emit a stderr breadcrumb naming the dropped
    # count so a vanished-state debugging session can see it happened, rather
    # than the entries disappearing silently. The count uses the SAME seed step
    # both merge programs do (a legacy single-slot failing record becomes one
    # `entries` slot under its file/__global__), so a clobbered legacy global
    # record is counted too — not reported as 0.
    local _dropped
    _dropped=$(jq -r --arg f "$file" '
      (if (.entries? | type) == "object" then .entries
       elif (.status // "") == "failing" then { (.file // "__global__"): {} }
       else {} end)
      | keys | map(select(. != $f)) | length' <<< "$existing" 2>/dev/null || echo "")
    if [ -n "$_dropped" ] && [ "$_dropped" -gt 0 ] 2>/dev/null; then
      printf 'gate-file: primary write failed at %s; single-slot fallback dropped %s other entry(ies)\n' \
        "$gate_file" "$_dropped" >&2
      # stderr is easily lost in a hook-driven pipeline; also append a durable
      # breadcrumb beside the gate file so a vanished-state investigation has
      # something to read. Best-effort: never let a logging failure abort the write.
      printf '%s primary write failed; single-slot fallback dropped %s entry(ies)\n' \
        "$now" "$_dropped" >> "${gate_file}.dropped.log" 2>/dev/null || true
    fi
    jq -n --arg reason "$reason" --arg source "$source" --arg file "$file" \
      --arg violations "$violations" --arg updatedAt "$now" \
      '{status: "failing", reason: $reason, source: $source, file: $file,
        violations: $violations, updatedAt: $updatedAt}' > "$gate_file" 2>/dev/null || true
  fi

  # gate_fail telemetry: this function always ends in an attempted write (primary
  # or fallback), so it always represents a failure being recorded — one event
  # per call, best-effort (never affects the write above).
  if _gate_file_ensure_telemetry; then
    telemetry_append "$(_gate_file_root "$gate_file")" "gate_fail" \
      "$(jq -cn --arg file "$file" --arg source "$source" '{file: $file, source: $source}')"
  fi
}

# gate_clear_file GATE_FILE FILE SOURCE
# Removes this file's entry IF this source owns it. Other entries are promoted
# back to the top level (gate stays failing); "passing" is written only when
# no entry remains. A failing record owned by another source/file is left
# untouched, matching the old single-slot clear semantics.
gate_clear_file() {
  local gate_file="$1" file="$2" source="$3"
  local existing now owns tmp
  [ -f "$gate_file" ] || return 0
  existing=$(cat "$gate_file" 2>/dev/null) || return 0
  # A malformed gate file can't be parsed, so a clear silently no-ops and the
  # gate stays stuck failing until the next gate_record_failure rewrites it.
  # Emit a breadcrumb so that stuck state is debuggable, not invisible.
  if ! jq -e . <<< "$existing" >/dev/null 2>&1; then
    printf 'gate-file: malformed JSON at %s; ignoring clear (gate stays failing until next write)\n' "$gate_file" >&2
    return 0
  fi
  [ "$(jq -r '.status // ""' <<< "$existing" 2>/dev/null)" = "failing" ] || return 0
  owns=$(jq -r --arg f "$file" --arg s "$source" '
    if (.entries? | type) == "object"
    then (((.entries[$f]? // {}) | .source // "") == $s)
    else ((.source // "") == $s and (.file // "") == $f)
    end' <<< "$existing" 2>/dev/null)
  [ "$owns" = "true" ] || return 0
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  tmp=$(mktemp "${gate_file}.XXXXXX" 2>/dev/null) || return 0
  if jq --arg file "$file" --arg source "$source" --arg now "$now" '
    ((if (.entries? | type) == "object" then .entries
      elif (.status // "") == "failing" then
        { (.file // "__global__"): {
            source: (.source // ""), reason: (.reason // ""),
            violations: (.violations // ""), updatedAt: (.updatedAt // $now) } }
      else {} end) | del(.[$file])) as $left
    | if ($left | length) == 0
      then { status: "passing", source: $source, updatedAt: $now }
      else (($left | to_entries | sort_by(.value.updatedAt // "", .key)) as $sorted
        | ($sorted | last) as $latest
        | { status: "failing",
            reason: ($latest.value.reason // "Quality gate failing"),
            source: ($latest.value.source // ""),
            file: $latest.key,
            violations: ([$sorted[] | (.value.violations // "")] | join("")),
            entries: $left, updatedAt: $now })
      end
  ' <<< "$existing" > "$tmp" 2>/dev/null; then
    mv -f "$tmp" "$gate_file"
    # gate_clear telemetry: ONLY here, after the write actually lands — every
    # early return above (no file / unreadable / malformed / not failing /
    # other-source entry) emits nothing, and so does a mktemp or jq failure
    # below this point (the `else` branch), since no transition happened.
    if _gate_file_ensure_telemetry; then
      telemetry_append "$(_gate_file_root "$gate_file")" "gate_clear" \
        "$(jq -cn --arg file "$file" --arg source "$source" '{file: $file, source: $source}')"
    fi
  else
    rm -f "$tmp"
  fi
}
