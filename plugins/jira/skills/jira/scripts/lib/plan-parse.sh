#!/usr/bin/env bash
# Plan-doc parsing for the `plan` family: pull the machine-readable steps block
# and the `**Issue:**` header key out of a jira plan doc. Pure functions, jq-only,
# no command execution and no `set` (this is a sourced library).
# Source via:   . "${BASH_SOURCE[0]%/*}/plan-parse.sh"
#
# The steps-block grammar mirrors toolu's plan-ledger-parse.sh so a jira plan doc
# stays readable by the same tooling: an exact `## Steps (machine-readable)`
# heading followed by the first ```json fence.

# jira_plan_parse_steps PLAN_DOC
# Print the normalized steps JSON array (jq-compacted) on stdout. Validates the
# block is a non-empty array whose every element has non-empty string
# id/title/check. Backfills the optional authored fields (activity -> null).
# Missing heading/block, empty array, or malformed json -> stderr + non-zero.
jira_plan_parse_steps() {
  local doc="$1" block
  if [ ! -f "$doc" ]; then
    echo "jira plan: plan doc not found: $doc" >&2
    return 1
  fi
  block=$(awk '
    /^## Steps \(machine-readable\)[[:space:]]*$/ { in_steps = 1; next }
    in_steps && !in_block && /^```json[[:space:]]*$/ { in_block = 1; next }
    in_block && /^```[[:space:]]*$/ { exit }
    in_block { print }
  ' "$doc")
  if [ -z "$block" ]; then
    echo "jira plan: no '## Steps (machine-readable)' json block in $doc" >&2
    return 1
  fi
  if ! jq -e '
    type == "array"
    and length > 0
    and all(.[];
      (.id    | type == "string" and length > 0) and
      (.title | type == "string" and length > 0) and
      (.check | type == "string" and length > 0))
  ' <<< "$block" >/dev/null 2>&1; then
    echo "jira plan: steps block in $doc is not a non-empty array of {id,title,check} strings" >&2
    return 1
  fi
  jq -c 'map(.activity = (.activity // null))' <<< "$block"
}

# jira_plan_doc_field DOC FIELD
# Print the trimmed value of an inline-bold `**<FIELD>:**` from a packed doc
# header line, or empty string when absent. Never errors — a missing field is a
# valid signal, not a failure. Same extraction as toolu's pl_doc_field.
jira_plan_doc_field() {
  local doc="$1" field="$2"
  [ -n "$doc" ] && [ -f "$doc" ] || return 0
  [ -n "$field" ] || return 0
  grep -m1 -F "**${field}:**" "$doc" 2>/dev/null \
    | sed -E "s/.*\\*\\*${field}:\\*\\*[[:space:]]*//; s/[[:space:]]+\\*\\*[^*]+:\\*\\*.*$//; s/^[[:space:]]+//; s/[[:space:]]+$//"
}

# jira_plan_issue_key DOC
# Print the issue key declared by the doc's `**Issue:** <KEY>` header. A key must
# look like a Jira key (LETTERS-DIGITS); anything else is rejected so a malformed
# header can never steer the ledger filename.
jira_plan_issue_key() {
  local doc="$1" key
  key=$(jira_plan_doc_field "$doc" "Issue")
  if [[ ! "$key" =~ ^[A-Za-z][A-Za-z0-9_]*-[0-9]+$ ]]; then
    echo "jira plan: doc is missing a valid '**Issue:** <KEY>' header: $doc" >&2
    return 1
  fi
  printf '%s\n' "$key"
}
