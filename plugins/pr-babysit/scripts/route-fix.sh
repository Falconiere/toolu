#!/usr/bin/env bash
# route-fix.sh — tier this round's Fix items and route each tier group to a
# fixer host, model and effort.
#
# Usage: route-fix.sh --items <file> --host claude|codex [--state-file <path>]
#                     [--raise <itemId>]... [--no-jev] [--jev-answers-in <file>]
#                     [--now <iso8601>]
#
#   --items           the controller's items file: {round, items:[{id, kind,
#                     task, path?, line?, severity?, quote?}]}
#   --host            the controller host; picks the config files and the
#                     default pool
#   --state-file      slot state; only .hostCooldowns is read
#   --raise           lift that item one tier (capped at critical)
#   --jev-answers-in  replay a captured Jev answer map instead of calling Jev
#                     (tests, debugging — the workflow never passes it)
#   --now             pin the clock used for cooldowns
#
# Jev scores each item from its task, path, severity and kind; the heuristic
# (no Jev) reads task and severity. The reviewer's quote is untrusted and never
# steers routing — it only reaches the fixer's brief, fenced as data.
# Tier = clamp(round(score + 0.15), 0, 3), epic-orchestrator's tierFromScore.
# Prints the route JSON on stdout. Exit: 0 · 2 usage · 3 structured error.
set -euo pipefail

PB_SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck source=lib/common.sh
. "$PB_SCRIPT_DIR/lib/common.sh"
# shellcheck source=lib/hosts.sh
. "$PB_SCRIPT_DIR/lib/hosts.sh"
# shellcheck source=lib/config.sh
. "$PB_SCRIPT_DIR/lib/config.sh"

items_file=""; host=""; state_file=""; raise="[]"; use_jev=1; answers_in=""; now=""
while [ $# -gt 0 ]; do
  case "$1" in
    --items)          items_file="${2:-}"; shift 2 ;;
    --host)           host="${2:-}"; shift 2 ;;
    --state-file)     state_file="${2:-}"; shift 2 ;;
    --raise)          raise=$(jq -c --arg id "${2:-}" '. + [$id]' <<<"$raise"); shift 2 ;;
    --no-jev)         use_jev=0; shift ;;
    --jev-answers-in) answers_in="${2:-}"; shift 2 ;;
    --now)            now="${2:-}"; shift 2 ;;
    *) pb_fail usage "route-fix.sh: unknown argument: $1" ;;
  esac
done
pb_require jq
pb_init
[[ "$host" =~ ^(claude|codex)$ ]] || pb_fail usage "route-fix.sh: --host claude|codex required"
[ -n "$items_file" ] || pb_fail usage "route-fix.sh: --items required"
[ -n "$now" ] || now=$(pb_now)

# ---- items: a non-empty list of uniquely-identified tasks
pb_json_valid "$items_file" || pb_fail plan_invalid "items file is missing or not JSON: $items_file"
problem=$(jq -r --argjson raise "$raise" '
  (.items // null) as $it
  | if ($it | type) != "array" or ($it | length) == 0 then "items must be a non-empty array"
    elif any($it[]; (.id | type) != "string" or .id == "") then "every item needs an id"
    elif ($it | map(.id) | unique | length) != ($it | length) then "item ids must be unique"
    elif any($it[]; (.task | type) != "string" or .task == "") then "every item needs a task"
    elif any($it[]; .kind as $k | (["thread","conversation","review","ci"] | index([$k])) == null) then "item kind must be thread, conversation, review or ci"
    elif any($raise[]; . as $r | ($it | map(.id) | index([$r])) == null) then "--raise names an item that is not in the items file"
    else "" end' "$items_file")
[ -z "$problem" ] || pb_fail plan_invalid "$problem"

config=""
pb_capture config pb_config_load "$host"
cooldowns='{}'
if [ -n "$state_file" ] && pb_json_valid "$state_file"; then
  cooldowns=$(jq -c '.hostCooldowns // {}' "$state_file")
fi

# ---- hosts whose CLI is on PATH (an environment observation, like the clock)
missing='[]'
for h in $(jq -r '.hosts[]' <<<"$config"); do
  command -v "$(pb_host_cli "$h")" >/dev/null 2>&1 || missing=$(jq -c --arg h "$h" '. + [$h]' <<<"$missing")
done

# ---- scores: replayed answers, a live Jev call, or none (heuristic)
answers='{}'; jev_note=""
jev_path() {
  local p
  for p in "${PB_JEV:-}" "$(pb_agent_home claude)/jev/jev.sh" "$(pb_agent_home codex)/jev/jev.sh"; do
    [ -n "$p" ] && [ -f "$p" ] && { printf '%s\n' "$p"; return 0; }
  done
  return 1
}
if [ -n "$answers_in" ]; then
  pb_json_valid "$answers_in" || pb_fail usage "route-fix.sh: --jev-answers-in is not JSON: $answers_in"
  answers=$(jq -c . "$answers_in")
elif [ "$use_jev" -eq 0 ] || [ "$(jq -r '.jev' <<<"$config")" = false ]; then
  jev_note="jev disabled; heuristic tiers used"
elif ! jev=$(jev_path); then
  jev_note="jev unavailable (jev.sh not installed); heuristic tiers used"
elif [ -z "${TYPESAFE_API_KEY:-}" ]; then
  jev_note="jev unavailable (TYPESAFE_API_KEY not set); heuristic tiers used"
else
  pb_mktmpdir
  jq -c '{items: (.items | map({key: .id, value: {task, path: (.path // null), severity: (.severity // null), kind}}) | from_entries)}' \
    "$items_file" >"$PB_TMPDIR/state.json"
  jq -c '.items | map({key: .id, value: {type: "score",
      instructions: ("How much implementation complexity does the review fix `items[\"" + .id + "\"]` carry for one coding agent? Judge from its task, path, severity and kind only."),
      criteria: [
        "Trivial: a typo, wording, rename, import, or one-line change with an obvious answer.",
        "Standard: a bounded edit in one function or file with a clear answer, possibly with its colocated test.",
        "Complex: spans several files, or needs new tests, concurrency handling, or behavior design.",
        "Critical: security or data-loss risk, cross-cutting architecture, or an ambiguous request that needs deep reasoning."]}})
    | from_entries' "$items_file" >"$PB_TMPDIR/questions.json"
  rc=0
  bash "$jev" ask "$PB_TMPDIR/questions.json" -s "@$PB_TMPDIR/state.json" >"$PB_TMPDIR/answers.json" 2>"$PB_TMPDIR/jev.err" || rc=$?
  if [ "$rc" -ne 0 ]; then
    jev_note="jev failed (exit $rc): $(head -c 160 "$PB_TMPDIR/jev.err" | tr '\n' ' '); heuristic tiers used"
  elif ! jq -e 'type == "object"' "$PB_TMPDIR/answers.json" >/dev/null 2>&1; then
    jev_note="jev returned an unparsable answer; heuristic tiers used"
  else
    answers=$(jq -c . "$PB_TMPDIR/answers.json")
  fi
fi

# ---- tiers, groups, hosts
jq -c --argjson config "$config" --argjson answers "$answers" --argjson raise "$raise" \
  --argjson cooldowns "$cooldowns" --argjson missing "$missing" --arg now "$now" --arg jevNote "$jev_note" '
  def tiers: ["trivial","standard","complex","critical"];
  def classes: ["mechanical","implementation","architecture","architecture"];
  def heuristic:
    ((.severity // "") | ascii_downcase) as $sev
    | ((.task // "") | ascii_downcase) as $text
    | if $sev == "critical" or ($text | test("\\b(security|injection|race|concurrency|deadlock|data[- ]loss)\\b")) then 3
      elif $sev == "high" then 2
      elif (["low","nit",""] | index([$sev])) != null
           and ($text | test("\\b(typos?|nits?|wording|renam(e|es|ed|ing)|spelling|whitespace|comments?|docs?|imports?)\\b")) then 0
      else 1 end;
  def tier_of($score): [0, ([3, ($score + 0.15 + 0.5 | floor)] | min)] | max;
  ($now | fromdateiso8601) as $t
  | ($cooldowns | to_entries | map(select((.value.until // "") != "" and (.value.until | fromdateiso8601) > $t) | .key)) as $cooling
  | ($config.hosts - $missing - $cooling) as $eligible
  | [ .items[] | . as $i | ($answers[$i.id].score // null) as $s
      | (if ($s | type) == "number" then {tier: tier_of($s), score: $s, confidence: ($answers[$i.id].confidence // null), source: "jev"}
         else {tier: ($i | heuristic), score: null, confidence: null, source: "heuristic"} end) as $r
      | (($raise | index([$i.id])) != null) as $raised
      | {id: $i.id, tier: (if $raised then ([$r.tier + 1, 3] | min) else $r.tier end), score: $r.score,
         confidence: $r.confidence, raised: $raised, source: $r.source} ] as $scored
  | ($scored | map(.source) | unique) as $sources
  | (if $config.dispatch == "inline" then "prBabysit.dispatch is inline"
     elif ($eligible | length) == 0 then "no fixer host available"
       + (if ($cooling | length) > 0 then " (cooling: " + ($cooling | join(", ")) + ")" else "" end)
       + (if ($missing | length) > 0 then " (CLI not on PATH: " + ($missing | join(", ")) + ")" else "" end)
     else null end) as $inlineWhy
  | (if $inlineWhy == null then "herdr" else "inline" end) as $dispatch
  | [ $scored | group_by(.tier) | reverse | to_entries[]
      | (.value[0].tier) as $tier
      | (if $dispatch == "inline" then null
         else ((($config.prefer[tiers[$tier]] // []) | map(select(. as $h | $eligible | index([$h]))))[0] // $eligible[0]) end) as $h
      | ($config.routing[$h // ""][$tier] // {}) as $choice
      | {seq: (.key + 1), tier: tiers[$tier], class: classes[$tier], host: $h,
         model: (if $h then $choice.model // null else null end),
         effort: (if $h then $choice.effort // null else null end),
         items: (.value | map(.id))} ] as $groups
  | {version: 1, dispatch: $dispatch, unattended: $config.unattended,
     source: (if $sources == ["jev"] then "jev" elif $sources == ["heuristic"] then "heuristic" else "mixed" end),
     note: ([$jevNote, $inlineWhy,
             (if ($missing | length) > 0 and ($eligible | length) > 0 then "dropped (CLI not on PATH): " + ($missing | join(", ")) else empty end),
             (if ($cooling | length) > 0 and ($eligible | length) > 0 then "cooling: " + ($cooling | join(", ")) else empty end)]
            | map(select(. != null and . != "")) | if length == 0 then null else join("; ") end),
     items: ($scored | map(.tier |= tiers[.])), groups: $groups}' "$items_file"
