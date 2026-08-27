#!/usr/bin/env bash
# parse-verdict.sh — turn the CI review bot's issue comment (stdin) into a
# deterministic JSON verdict the babysit loop acts on. Keeps the parsing OUT of
# the LLM prompt so behaviour is testable (scripts/__tests__/parse-verdict.bats
# against a captured real comment) and stable.
#
# stdin : raw comment body (markdown)
# stdout: { is_review_comment, state, complete, verdict, verdict_label, findings[], must_fix[] }
#   state: in_progress | complete | unknown
#     - unknown  → no checkbox checklist found (cannot judge completeness; the
#                  caller degrades to GitHub check-conclusion behaviour)
#     - in_progress → ≥1 unchecked `- [ ]` (review still running; do not act)
#     - complete → ≥1 checkbox AND none unchecked
#   findings[]: { path, line|null, severity, text, key }  (key = path:line:sha1(text)[:8])
#   must_fix[]:  free-text lines from `### Top-N must-fix`
#
# must_fix is NOT a duplicate of findings. The bot populates the two sections
# independently, and they disagree in both directions: a review has reported
# `Findings (0)` while Top-N listed three actionable items, and an `approved`
# verdict has shipped with Top-N still populated. A caller that reads only
# findings[] therefore both misses work and over-trusts a pass. These are plain
# sentences, not `path:line` findings, so they carry no key and cannot be
# resolved as threads — surface them to a human rather than acting on them
# mechanically.
#
# Identification is by MARKER, robust to both header states the bot uses
# ("PR Review in Progress" → "Code Review —"): a CI job link, a "Code Review"
# header, or an "agent-merge" verdict label. No marker → is_review_comment:false.
set -o pipefail

command -v jq >/dev/null 2>&1 || { echo "parse-verdict.sh: jq required" >&2; exit 2; }

input=$(cat 2>/dev/null || true)

_empty() { jq -nc '{is_review_comment:false, state:"unknown", complete:false, verdict:"none", verdict_label:"", findings:[]}'; }

# Empty / unreadable stdin → not a review comment.
[ -n "${input//[[:space:]]/}" ] || { _empty; exit 0; }

# --- Identify: marker-based, both states ---
# Precondition: the babysit pre-filters comments to claude[bot]/github-actions[bot]
# before calling this; the markers below are anchored to the bot's own structures
# (review headings, the [View job] CI link, a backticked agent-merge label) so a
# stray "actions/runs/…" or "agent-merge-…" in arbitrary prose won't false-positive.
is_review=false
if printf '%s' "$input" | grep -qE '^### Code Review|^### PR Review in Progress|\[View job\]\([^)]*actions/runs/[0-9]+|`agent-merge-[a-z]|^`(merge-approved|request-changes)`$'; then
  is_review=true
fi
if [ "$is_review" != true ]; then _empty; exit 0; fi

# --- Completeness: checkbox state only ---
unchecked=$(printf '%s\n' "$input" | grep -cE '^[[:space:]]*-[[:space:]]\[[[:space:]]\]' || true)
checked=$(printf '%s\n'   "$input" | grep -cE '^[[:space:]]*-[[:space:]]\[[xX]\]'        || true)
boxes=$((unchecked + checked))
if   [ "$boxes" -eq 0 ];     then state="unknown"
elif [ "$unchecked" -gt 0 ]; then state="in_progress"
else                              state="complete"
fi
complete=false; [ "$state" = complete ] && complete=true

# --- Verdict --- the machine-readable label is AUTHORITATIVE.
# Don't infer from prose when a label exists: finding TEXT routinely discusses
# "Changes requested"/"approved" (e.g. a finding about verdict parsing itself),
# and a whole-body grep would misclassify an approved PR as "changes". Fall back
# to prose only when no label is present (changes-first there, as the safe bias).
#
# The label has two historical shapes and BOTH must parse:
#   legacy (<=PR#31): `agent-merge-approved` / `agent-merge-blocked`
#   current:          `merge-approved` / `request-changes`
# Resolution order matters. The checklist line `- [x] Set verdict label (`X`)`
# is preferred because it is emitted before the findings section, so a finding
# that quotes a label verbatim cannot shadow it. Next the standalone trailing
# label chip (current shape emits only that). Only then a bare legacy scan.
verdict_label=$(printf '%s\n' "$input" \
  | grep -oE 'Set verdict label \(`[^`]+`\)' | head -1 | grep -oE '`[^`]+`' | tr -d '`' || true)
if [ -z "$verdict_label" ]; then
  verdict_label=$(printf '%s\n' "$input" \
    | grep -oE '^`(agent-merge-[a-z-]+|merge-approved|request-changes)`$' | tail -1 | tr -d '`' || true)
fi
if [ -z "$verdict_label" ]; then
  verdict_label=$(printf '%s' "$input" | grep -oE 'agent-merge-[a-z-]+' | head -1 || true)
fi

# `merge-approved` contains "approved"; `request-changes` contains "changes";
# `agent-merge-blocked` contains "blocked". Test approved first so the shared
# substring "merge" in either label can never decide the branch.
if [[ "$verdict_label" == *approved* ]]; then
  verdict="approved"
elif [[ "$verdict_label" == *blocked* || "$verdict_label" == *changes* ]]; then
  verdict="changes"
else
  # No label. Prefer the single `**Verdict:**` summary line over a whole-body
  # grep: the current bot bolds the KEY (`**Verdict:** ✅ Approved`), not the
  # word, so the legacy `**Approved**` pattern never matches it.
  vline=$(printf '%s\n' "$input" | grep -m1 -E '^\*\*Verdict:\*\*' || true)
  if   printf '%s' "$vline" | grep -qiE 'changes requested'; then verdict="changes"
  elif printf '%s' "$vline" | grep -qiE 'approved';          then verdict="approved"
  elif printf '%s' "$input" | grep -qiE '\*\*Changes requested\*\*|changes-requested'; then verdict="changes"
  elif printf '%s' "$input" | grep -qiE '\*\*Approved\*\*'; then verdict="approved"
  else verdict="none"
  fi
fi

# --- Findings: only the `### Findings` … next `### ` block, lines of the form
#     `path[:line]`: severity: text
_sha1() { (sha1sum 2>/dev/null || shasum 2>/dev/null || echo nohash) | cut -c1-8; }
# Tolerate a decorated header (`### Findings`, `### Findings (6)`) — exact-match
# would miss a count suffix and silently report zero findings.
findings_block=$(printf '%s\n' "$input" | awk '/^### Findings([[:space:]]|$)/{f=1;next} /^### /{f=0} f')
findings_json="[]"
while IFS= read -r line; do
  [[ "$line" =~ ^\`([^\`]+)\`:\ (blocker|high|medium|low|nit):\ (.*)$ ]] || continue
  raw_path="${BASH_REMATCH[1]}"; sev="${BASH_REMATCH[2]}"; text="${BASH_REMATCH[3]}"
  if [[ "$raw_path" =~ ^(.+):([0-9]+)$ ]]; then path="${BASH_REMATCH[1]}"; ln="${BASH_REMATCH[2]}"; else path="$raw_path"; ln=""; fi
  h=$(printf '%s' "$text" | _sha1)
  key="${path}:${ln}:${h}"
  obj=$(jq -nc --arg path "$path" --arg line "$ln" --arg severity "$sev" --arg text "$text" --arg key "$key" \
    '{path:$path, line:(if $line=="" then null else ($line|tonumber) end), severity:$severity, text:$text, key:$key}') || continue
  findings_json=$(jq -c --argjson o "$obj" '. + [$o]' <<<"$findings_json")
done <<< "$findings_block"

# --- Top-N must-fix: free-text lines until the next heading or <details>.
# Same decorated-header tolerance as Findings above; entries may be bare lines
# or markdown list items.
must_fix_block=$(printf '%s\n' "$input" | awk '/^### Top-N must-fix([[:space:]]|$)/{f=1;next} /^### |^<details>/{f=0} f')
must_fix_json="[]"
while IFS= read -r line; do
  line="${line#"${line%%[![:space:]]*}"}"
  line="${line%"${line##*[![:space:]]}"}"
  [ -n "$line" ] || continue
  # Strip a leading list marker only — "- " or "1. ". A blanket "first word"
  # strip ate the first word of every unmarked line ("Fix the silent
  # swallowing..." became "the silent swallowing...").
  line="${line#- }"
  line="${line#\* }"
  [[ "$line" =~ ^[0-9]+\.[[:space:]]+(.*)$ ]] && line="${BASH_REMATCH[1]}"
  [ -n "$line" ] || continue
  must_fix_json=$(jq -c --arg t "$line" '. + [$t]' <<<"$must_fix_json") || continue
done <<< "$must_fix_block"

jq -nc \
  --argjson is_review "$is_review" \
  --arg state "$state" \
  --argjson complete "$complete" \
  --arg verdict "$verdict" \
  --arg verdict_label "${verdict_label:-}" \
  --argjson findings "$findings_json" \
  --argjson must_fix "$must_fix_json" \
  '{is_review_comment:$is_review, state:$state, complete:$complete, verdict:$verdict, verdict_label:$verdict_label, findings:$findings, must_fix:$must_fix}'
