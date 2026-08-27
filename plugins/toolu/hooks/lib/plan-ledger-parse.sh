#!/usr/bin/env bash
# Plan-ledger parse + I/O lib: extract the machine-readable steps block from a
# plan doc and read/write the per-branch ledger json atomically. Pure functions,
# jq-only, no command execution and no `set` (this is a sourced library).
# Source via:   . "${BASH_SOURCE%/*}/plan-ledger-parse.sh"

# pl_parse_steps PLAN_DOC_PATH
# Extract the FIRST ```json fenced block under the `## Steps (machine-readable)`
# heading and print the JSON array to stdout. Validates with jq: must be a
# non-empty array whose every element has non-empty string id/title/check, and
# whose optional `model` (the tier that should execute the step) is one of
# haiku|sonnet|opus|fable|inherit.
# On missing heading/block, empty array, malformed json, or an unroutable
# model: error to stderr, non-zero return, nothing on stdout. Never writes.
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
  # `model` is the tier that should execute the step. Optional, but when present
  # it must be a routable alias — a typo here would send `execution` to delegate
  # on a model the Agent tool rejects, failing the step for a reason that has
  # nothing to do with the step. Reject at parse time instead.
  # The alias list mirrors TOOLU_MODEL_ALIASES in hooks/lib/config.sh; this lib
  # stays jq-only rather than sourcing the config loader, so parity is enforced
  # by __tests__/plan-ledger-model.bats.
  local bad_models
  if ! bad_models=$(jq -r '
    [ .[] | select(.model != null)
          | select((.model | type) != "string"
                   or ((.model) as $m
                       | ["haiku","sonnet","opus","fable","inherit"] | index($m) | not))
          | "\(.id)=\(.model|tostring)" ] | join(", ")
  ' <<< "$block" 2>/dev/null); then
    # Fail closed: an unvalidated model would reach the executor as a delegation
    # target, so an internal validator failure is an error, not a pass.
    echo "plan-ledger-parse: could not validate step models in $doc" >&2
    return 1
  fi
  if [ -n "$bad_models" ]; then
    echo "plan-ledger-parse: invalid step model in $doc ($bad_models); allowed: haiku sonnet opus fable inherit" >&2
    return 1
  fi
  # `paths` narrows what a step's freshness depends on. A step that declares it
  # is re-run only when those paths change, instead of on every change anywhere
  # in the branch diff. Wrong contents here would silently keep a step green
  # after its real inputs moved, so the shape is checked rather than trusted:
  # an array of non-empty strings, or absent.
  local bad_paths
  if ! bad_paths=$(jq -r '
    [ .[] | select(.paths != null)
          | select((.paths | type) != "array"
                   or ([.paths[] | select((type) != "string" or (. == ""))] | length) > 0)
          | .id ] | join(", ")
  ' <<< "$block" 2>/dev/null); then
    echo "plan-ledger-parse: could not validate step paths in $doc" >&2
    return 2
  fi
  if [ -n "$bad_paths" ]; then
    echo "plan-ledger-parse: invalid step paths in $doc ($bad_paths); expected an array of non-empty strings" >&2
    return 2
  fi

  # Emit the normalized array (jq-compacted) on stdout. Backfill the optional
  # authored fields so downstream serialization always sees them: ac_refs,
  # depends_on and paths default to [], input and model to null. Legacy
  # {id,title,check} steps thus gain the defaults; authored values are preserved.
  # Permissive `// default` also coerces a present-but-null field to its default.
  jq -c 'map(
    .ac_refs    = (.ac_refs // []) |
    .depends_on = (.depends_on // []) |
    .paths      = (.paths // []) |
    .input      = (.input // null) |
    .model      = (.model // null)
  )' <<< "$block"
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

# pl_parse_acs SPEC_DOC
# Print the newline-separated list of acceptance-criterion ids (`AC-<n>`) declared
# under the spec's `## Acceptance criteria` heading, in document order, deduped.
# Ids come from the stable `**AC-<n>:**` bold prefix. A missing doc, a doc with no
# such heading, or a heading with no AC lines all yield an empty list and a zero
# return — "no ACs" is a valid state, never an error. Never writes.
pl_parse_acs() {
  local doc="$1"
  [ -n "$doc" ] && [ -f "$doc" ] || return 0
  # awk state machine: arm inside the `## Acceptance criteria` section, disarm at
  # the next `## ` heading, and emit the id from each `**AC-<n>:**` bold prefix.
  # `seen[]` dedups while preserving first-seen order.
  awk '
    /^## Acceptance criteria[[:space:]]*$/ { in_ac = 1; next }
    in_ac && /^## / { in_ac = 0 }
    in_ac {
      if (match($0, /\*\*AC-[0-9]+:\*\*/)) {
        id = substr($0, RSTART + 2, RLENGTH - 5)   # strip leading ** and trailing :**
        if (!(id in seen)) { seen[id] = 1; print id }
      }
    }
  ' "$doc"
}

# pl_check_ac_refs PLAN_DOC [SPEC_DOC]
# Print each dangling ac_ref — an `ac_refs` id in the plan's steps block that is
# NOT declared in the spec — one per line, and return non-zero iff any exist.
# When SPEC_DOC is omitted, empty, or the literal "none" (case-insensitive), the
# plan is spec-less: coverage is skipped, nothing is printed, return 0. A spec
# present but lacking an `## Acceptance criteria` section declares zero ids, so
# every non-empty ac_ref is dangling. Parse failure of the plan -> non-zero.
pl_check_ac_refs() {
  local plan="$1" spec="${2:-}" steps refs acs dangling
  # Spec-less: nothing to resolve against -> pass with no dangling refs.
  case "$(printf '%s' "$spec" | tr '[:upper:]' '[:lower:]')" in
    ""|none) return 0 ;;
  esac
  steps=$(pl_parse_steps "$plan") || return 1
  # All referenced ids across every step, deduped (order irrelevant for set diff).
  refs=$(jq -r '[.[].ac_refs[]?] | unique[]' <<< "$steps") || return 1
  # No refs at all -> nothing can dangle.
  [ -n "$refs" ] || return 0
  acs=$(pl_parse_acs "$spec")
  # Set difference: refs not present in the spec's declared ids. comm needs both
  # operands sorted; refs is already unique. Empty `acs` -> all refs dangle.
  dangling=$(comm -23 <(printf '%s\n' "$refs" | sort) <(printf '%s\n' "$acs" | sort)) || return 1
  if [ -n "$dangling" ]; then
    printf '%s\n' "$dangling"
    return 1
  fi
  return 0
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
