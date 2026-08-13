#!/usr/bin/env bash
# Ledger store for the `plan` family: resolve the ledger path, merge authored
# steps over persisted status, recompute the summary, and write atomically.
# Pure functions, jq-only, no command execution and no `set` (sourced library).
# Source via:   . "${BASH_SOURCE[0]%/*}/plan-store.sh"
#
# Emits schema version 1, the same document plugins/toolu/dashboard/state.ts
# reads. Two fields are deliberate and load-bearing:
#   base_branch: ""   -> dashboard currentDiffSha() returns null, so no Jira step
#                        is ever marked `stale` when unrelated code commits move
#                        the repo diff (state.ts: `if (!base) return null`).
#   diff_sha:    null -> a Jira step's freshness is not a function of the code diff.
# The file is named jira-<KEY>.json, never <branch-slug>.json, so toolu's push
# gate (which stats only "$ledger_dir/<branch-slug>.json") can never block a push
# on Jira state.

# jira_plan_repo_root
# Print the repo root, or the cwd when not inside a git work tree.
jira_plan_repo_root() {
  git rev-parse --show-toplevel 2>/dev/null || pwd
}

# This skill is self-contained and cannot source the toolu plugin's host
# adapter. Mirror only its host-native project directory decision here.
jira_plan_project_dirname() {
  if [ -n "${TOOLU_PROJECT_CONFIG_DIRNAME:-}" ]; then
    printf '%s\n' "$TOOLU_PROJECT_CONFIG_DIRNAME"
  elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || { [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ]; }; then
    printf '.codex\n'
  else
    printf '.claude\n'
  fi
}

# jira_plan_require_key KEY
# Accept only a well-formed Jira key (LETTERS-DIGITS). Both path builders below
# interpolate the key into a filename, so anything else -- a slash, a `..`, an
# absolute path -- must be rejected here rather than escaping the ledger dir.
jira_plan_require_key() {
  local key="${1:-}"
  if [[ ! "$key" =~ ^[A-Za-z][A-Za-z0-9_]*-[0-9]+$ ]]; then
    echo "jira plan: not a valid issue key: '${key}'" >&2
    return 1
  fi
}

# jira_plan_ledger_path KEY
# Print <repo_root>/<host-dir>/tmp/plan-ledger/jira-<KEY>.json.
jira_plan_ledger_path() {
  local key="$1" project_dir
  jira_plan_require_key "$key" || return 1
  project_dir=$(jira_plan_project_dirname)
  printf '%s\n' "$(jira_plan_repo_root)/${project_dir}/tmp/plan-ledger/jira-${key}.json"
}

# jira_plan_doc_path KEY
# Print <repo_root>/<host-dir>/tmp/jira/plans/<KEY>.md.
jira_plan_doc_path() {
  local key="$1" project_dir
  jira_plan_require_key "$key" || return 1
  project_dir=$(jira_plan_project_dirname)
  printf '%s\n' "$(jira_plan_repo_root)/${project_dir}/tmp/jira/plans/${key}.md"
}

# jira_plan_now — ISO-8601 UTC, second precision.
jira_plan_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# jira_plan_evidence COMBINED_OUTPUT
# JSON-encode the last 10 lines of a check's combined stdout+stderr, capped to
# ~2000 bytes. Prints a quoted JSON string. Mirrors toolu's pl_evidence.
jira_plan_evidence() {
  printf '%s' "$1" | tail -n 10 | head -c 2000 | jq -Rs .
}

# jira_plan_read STATE_FILE
# Print the ledger json. Absent, empty, or unparseable -> non-zero, no output.
jira_plan_read() {
  local f="$1" content
  [ -s "$f" ] || return 1
  content=$(cat "$f" 2>/dev/null) || return 1
  jq -e . <<< "$content" >/dev/null 2>&1 || return 1
  printf '%s\n' "$content"
}

# jira_plan_write STATE_FILE JSON
# Validate, then write atomically (temp + mv), creating the parent dir.
jira_plan_write() {
  local f="$1" json="$2" dir tmp
  if ! jq -e . <<< "$json" >/dev/null 2>&1; then
    echo "jira plan: refusing to write invalid ledger json to $f" >&2
    return 1
  fi
  dir=$(dirname "$f")
  mkdir -p "$dir" || { echo "jira plan: cannot create ledger dir: $dir" >&2; return 1; }
  tmp="$f.tmp.$$"
  if ! jq . <<< "$json" > "$tmp" 2>/dev/null; then
    rm -f "$tmp"; echo "jira plan: failed to stage ledger to $tmp" >&2; return 1
  fi
  if ! mv "$tmp" "$f"; then
    rm -f "$tmp"; echo "jira plan: atomic mv failed for $f" >&2; return 1
  fi
}

# jira_plan_recompute LEDGER_JSON
# Recompute summary{} and next from steps[]. `stale` is always 0 and
# `fresh_green` equals `green`: Jira freshness does not depend on the code diff.
# `next` is the first step that is not green. Note the dashboard derives its
# status dot and progress from steps[] (aggregate.ts countSteps), not from this
# summary — summary must simply agree with steps[].
jira_plan_recompute() {
  jq '
    (.steps | length) as $total
    | (.steps | map(select(.status=="green"))  | length) as $green
    | (.steps | map(select(.status=="red"))    | length) as $red
    | (.steps | map(select(.status=="pending"))| length) as $pending
    | (.steps | map(select(.status=="running"))| length) as $running
    | .summary = { total: $total, green: $green, red: $red, pending: $pending,
                   running: $running, stale: 0, fresh_green: $green, retried: 0 }
    | .next = ((.steps | map(select(.status != "green")) | first | .id) // null)
  '
}

# jira_plan_build KEY PLAN_DOC STEPS_JSON [PREV_LEDGER_JSON]
# Build the full ledger document. Authored fields (title/check/activity) come
# from the doc; execution state (status/started_at/exit_code/last_run/
# evidence_tail) is carried over from PREV by step id, defaulting to pending.
# A step dropped from the doc disappears; a new one arrives pending.
jira_plan_build() {
  local key="$1" doc="$2" steps="$3" prev="${4:-null}" now
  now=$(jira_plan_now)
  jq -n \
    --argjson steps "$steps" \
    --argjson prev "$prev" \
    --arg branch "jira-${key}" \
    --arg doc "$doc" \
    --arg now "$now" '
    ((($prev.steps // []) | map({ (.id): . }) | add) // {}) as $old
    | {
        version: 1,
        branch: $branch,
        base_branch: "",
        plan_doc: $doc,
        updated_at: $now,
        summary: {},
        next: null,
        steps: ($steps | map(
          ($old[.id] // {}) as $p | {
            id, title, check,
            status:        ($p.status // "pending"),
            started_at:    ($p.started_at // null),
            activity:      (.activity // $p.activity // null),
            exit_code:     ($p.exit_code // null),
            diff_sha:      null,
            last_run:      ($p.last_run // null),
            evidence_tail: ($p.evidence_tail // null)
          }))
      }' | jira_plan_recompute
}

# jira_plan_set_step LEDGER_JSON ID FIELDS_JSON
# Merge FIELDS_JSON into the step with the given id, then recompute + restamp.
jira_plan_set_step() {
  local ledger="$1" id="$2" fields="$3"
  jq --arg id "$id" --argjson f "$fields" --arg now "$(jira_plan_now)" '
    .steps = (.steps | map(if .id == $id then . + $f else . end))
    | .updated_at = $now
  ' <<< "$ledger" | jira_plan_recompute
}
