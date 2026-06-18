#!/usr/bin/env bash
# Plan-ledger parse + I/O lib: extract the machine-readable steps block from a
# plan doc and read/write the per-branch ledger json atomically. Pure functions,
# jq-only, no command execution and no `set` (this is a sourced library).
# Source via:   . "${BASH_SOURCE%/*}/plan-ledger-parse.sh"

# pl_parse_steps PLAN_DOC_PATH
# Extract the FIRST ```json fenced block under the `## Steps (machine-readable)`
# heading and print the JSON array to stdout. Validates with jq: must be a
# non-empty array whose every element has non-empty string id/title/check.
# On missing heading/block, empty array, or malformed json: error to stderr,
# non-zero return, nothing on stdout. Never writes.
pl_parse_steps() {
  local doc="$1" block
  if [ ! -f "$doc" ]; then
    echo "plan-ledger-parse: plan doc not found: $doc" >&2
    return 1
  fi
  # awk state machine: once past the marker heading, capture the lines between
  # the next ```json fence and its closing ``` fence. `in_steps` arms after the
  # heading; `in_block` arms inside the first json fence; we stop at its close.
  block=$(awk '
    /^## Steps \(machine-readable\)[[:space:]]*$/ { in_steps = 1; next }
    in_steps && !in_block && /^```json[[:space:]]*$/ { in_block = 1; next }
    in_block && /^```[[:space:]]*$/ { exit }
    in_block { print }
  ' "$doc")
  if [ -z "$block" ]; then
    echo "plan-ledger-parse: no '## Steps (machine-readable)' json block in $doc" >&2
    return 1
  fi
  # Validate: parseable, non-empty array, every step has non-empty id/title/check.
  if ! jq -e '
    type == "array"
    and length > 0
    and all(.[];
      (.id    | type == "string" and length > 0) and
      (.title | type == "string" and length > 0) and
      (.check | type == "string" and length > 0))
  ' <<< "$block" >/dev/null 2>&1; then
    echo "plan-ledger-parse: steps block in $doc is not a non-empty array of {id,title,check} strings" >&2
    return 1
  fi
  # Emit the normalized array (jq-compacted) on stdout.
  jq -c . <<< "$block"
}

# pl_doc_field DOC FIELD
# Print the trimmed value of an inline-bold `**<FIELD>:**` from a packed doc
# header line, or empty string if the field (or doc) is absent. Never errors:
# a missing field is a valid "spec-less / unstamped" signal, not a failure.
#
# Header lines pack fields together, e.g.:
#   **Date:** 2026-06-17   **Status:** Draft   **Spec:** path.md   **Topic:** …
# Extraction (first matching line wins): strip up to and including `**FIELD:**`,
# then cut from the next `**Word:**` bold key (preceded by whitespace) or EOL,
# then trim surrounding whitespace. A trailing (last-on-line) field has no
# following bold key, so it runs to EOL. The boundary requires ≥1 space before
# the next `**…:**` so a value that happens to contain `**` mid-word is not
# truncated, and both the 3-space house style and a 1-space packing are handled.
pl_doc_field() {
  local doc="$1" field="$2"
  [ -n "$doc" ] && [ -f "$doc" ] || return 0
  [ -n "$field" ] || return 0
  # First line containing the bold key, then sed the value out of it.
  grep -m1 -F "**${field}:**" "$doc" 2>/dev/null \
    | sed -E "s/.*\\*\\*${field}:\\*\\*[[:space:]]*//; s/[[:space:]]+\\*\\*[^*]+:\\*\\*.*$//; s/^[[:space:]]+//; s/[[:space:]]+$//"
}

# pl_read_ledger STATE_FILE
# Print the ledger json to stdout. Empty or absent file -> non-zero, no output.
pl_read_ledger() {
  local state_file="$1" content
  if [ ! -s "$state_file" ]; then
    return 1
  fi
  content=$(cat "$state_file" 2>/dev/null) || return 1
  if ! jq -e . <<< "$content" >/dev/null 2>&1; then
    return 1
  fi
  printf '%s\n' "$content"
}

# pl_write_ledger STATE_FILE JSON_STRING
# Validate JSON_STRING with jq, then write it atomically (temp + mv) to
# STATE_FILE, creating the parent dir if needed. Invalid json -> non-zero,
# no write and no leftover temp file.
pl_write_ledger() {
  local state_file="$1" json="$2" dir tmp
  if ! jq -e . <<< "$json" >/dev/null 2>&1; then
    echo "plan-ledger-parse: refusing to write invalid ledger json to $state_file" >&2
    return 1
  fi
  dir=$(dirname "$state_file")
  if ! mkdir -p "$dir"; then
    echo "plan-ledger-parse: cannot create ledger dir: $dir" >&2
    return 1
  fi
  tmp="$state_file.tmp.$$"
  if ! jq . <<< "$json" > "$tmp" 2>/dev/null; then
    rm -f "$tmp"
    echo "plan-ledger-parse: failed to stage ledger to $tmp" >&2
    return 1
  fi
  if ! mv "$tmp" "$state_file"; then
    rm -f "$tmp"
    echo "plan-ledger-parse: atomic mv failed for $state_file" >&2
    return 1
  fi
}
