#!/usr/bin/env bash
# Plan-ledger checker lib + CLI: run each plan step's `check`, stamp mechanical
# status + content-addressed diff_sha into the per-branch ledger, and report
# fresh-green/next. The SCRIPT sets status (exit-code truth) — the agent cannot
# claim green. Sourceable (functions only) and runnable (guarded `main`).
# jq-only. Parse/IO errors fail closed (exit 2).
#
# Run:    bash plan-ledger.sh run <doc.md> [--step <id>] [--activity <label>]
#         bash plan-ledger.sh status | preflight [<doc.md>] | path | root | --self-test
# Source: . "${BASH_SOURCE%/*}/plan-ledger.sh"   (defines pl_* helpers, no run)

# pipefail so the `git diff | git hash-object` pipe surfaces failures instead of
# silently yielding the empty-blob sha. NOT -euo: this file sources libs and runs
# user `check` commands whose non-zero exits are expected signal, not fatal.
set -o pipefail

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}"
# shellcheck source=plan-ledger-parse.sh
. "$_toolu_lib/plan-ledger-parse.sh"
# shellcheck source=detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=plan-ledger-preflight.sh
. "$_toolu_lib/plan-ledger-preflight.sh"

# pl_diff_sha BASE
# Print the content-addressed diff hash of BASE...HEAD (matches push-review.sh:88).
# Empty stdout + non-zero on git failure so callers can fail closed.
pl_diff_sha() {
  local base="$1" sha
  sha=$(git diff --no-color "${base}...HEAD" 2>/dev/null | git hash-object --stdin 2>/dev/null) || return 1
  [ -n "$sha" ] || return 1
  printf '%s\n' "$sha"
}

# pl_ledger_path
# Print the ledger path for the current branch:
#   <project_root>/.claude/tmp/plan-ledger/<branch_slug>.json
# Non-zero if the project root can't be resolved (not a git repo).
pl_ledger_path() {
  local root branch slug
  root=$(detect_project_root)
  [ -n "$root" ] || return 1
  branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null) || return 1
  slug=$(branch_slug "$branch")
  printf '%s\n' "$root/.claude/tmp/plan-ledger/${slug}.json"
}

# pl_evidence COMBINED_OUTPUT
# JSON-encode the last 10 lines of a check's combined stdout+stderr, capped to
# ~2000 bytes, via `jq -Rs` (handles null bytes / invalid UTF-8 safely). Prints a
# JSON string (quoted) on stdout.
pl_evidence() {
  printf '%s' "$1" \
    | tail -n 10 \
    | head -c 2000 \
    | jq -Rs .
}

# pl_recompute LEDGER_JSON CURRENT_DIFF_SHA
# Recompute summary{total,green,red,pending,running,stale,fresh_green} and next
# against CURRENT_DIFF_SHA (a step is fresh-green iff status==green AND diff_sha
# matches; a running step is never fresh; next = first non-fresh-green step id,
# null when all fresh-green). Print the updated ledger json on stdout.
pl_recompute() {
  local ledger="$1" cur="$2"
  jq --arg cur "$cur" '
    def is_fresh: (.status == "green") and (.diff_sha == $cur);
    .summary = {
      total:       (.steps | length),
      green:       ([.steps[] | select(.status == "green")]  | length),
      red:         ([.steps[] | select(.status == "red")]    | length),
      pending:     ([.steps[] | select(.status == "pending")]| length),
      running:     ([.steps[] | select(.status == "running")]| length),
      stale:       ([.steps[] | select(.status == "green" and .diff_sha != $cur)] | length),
      fresh_green: ([.steps[] | select(is_fresh)] | length),
      retried:     ([.steps[] | select((.retries // []) | length > 0)] | length)
    }
    | .next = (first(.steps[] | select(is_fresh | not) | .id) // null)
  ' <<< "$ledger"
}

# pl_summary_line LEDGER_JSON SLUG
# Print the single-line, parseable summary:
#   plan-ledger <slug>: <fresh_green>/<total> fresh-green, next=<id|none>
pl_summary_line() {
  local ledger="$1" slug="$2"
  jq -r --arg slug "$slug" '
    "plan-ledger " + $slug + ": "
    + (.summary.fresh_green | tostring) + "/" + (.summary.total | tostring)
    + " fresh-green, next=" + (.next // "none")
  ' <<< "$ledger"
}

# pl_all_fresh LEDGER_JSON  ->  return 0 iff every step is fresh-green.
pl_all_fresh() {
  local ledger="$1"
  [ "$(jq -r '.next == null' <<< "$ledger")" = "true" ]
}

# pl_now  ->  UTC ISO-8601 timestamp.
pl_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# pl_build_step_entry STEPS_JSON ID STATUS EXIT_CODE DIFF_SHA EVIDENCE_JSON \
#                     [STARTED_AT] [ACTIVITY] [PRIOR_ENTRY_JSON]
# Print a single ledger step object merging the doc fields (id/title/check plus
# the authored ac_refs/depends_on/input from STEPS_JSON) with the run results.
# EVIDENCE_JSON is an already-JSON-encoded string. STARTED_AT/ACTIVITY are
# optional; an empty arg becomes JSON null. started_at is the ISO-8601 time the
# step entered `running`; activity is an optional short label.
#
# PRIOR_ENTRY_JSON is the step's prior on-disk ledger entry (or "" / "null" when
# none). retries[] is the chronological list of reds preceding this state: if the
# prior entry was red, it is archived as a retry record (attempt, exit_code,
# diff_sha, evidence_tail, at=prior.last_run) appended after the prior's own
# retries; otherwise the prior retries (or []) carry forward unchanged. Only reds
# are archived. All additive — version stays 1.
pl_build_step_entry() {
  local steps="$1" id="$2" status="$3" code="$4" sha="$5" evidence="$6"
  local started_at="${7:-}" activity="${8:-}" prior="${9:-}"
  [ -n "$prior" ] || prior="null"
  jq -n \
    --argjson steps "$steps" \
    --arg id "$id" \
    --arg status "$status" \
    --argjson code "$code" \
    --arg sha "$sha" \
    --arg now "$(pl_now)" \
    --argjson evidence "$evidence" \
    --arg started_at "$started_at" \
    --arg activity "$activity" \
    --argjson prior "$prior" '
    ($steps[] | select(.id == $id)) as $s
    | ($prior.retries // []) as $prior_retries
    | (if ($prior.status // null) == "red"
       then $prior_retries + [{
         attempt:       (($prior_retries | length) + 1),
         exit_code:     $prior.exit_code,
         diff_sha:      $prior.diff_sha,
         evidence_tail: $prior.evidence_tail,
         at:            $prior.last_run }]
       else $prior_retries end) as $retries
    | { id: $s.id, title: $s.title, check: $s.check,
        status: $status,
        started_at: (if $started_at == "" then null else $started_at end),
        activity: (if $activity == "" then null else $activity end),
        exit_code: $code, diff_sha: $sha,
        last_run: $now, evidence_tail: $evidence,
        ac_refs: ($s.ac_refs // []),
        depends_on: ($s.depends_on // []),
        input: ($s.input // null),
        retries: $retries }
  '
}

# pl_cmd_run DOC [--step ID] [--activity LABEL]
# Parse DOC's steps; cd to project root; run checks (all, or only --step ID
# preserving other entries from an existing ledger); recompute and write the
# ledger; print the summary line. Exit 0 iff all fresh-green, else 1; parse/IO
# error -> exit 2 (writes nothing).
#
# When --step is given, the step is written `running` + started_at (+ activity)
# in a first atomic write BEFORE its check runs, then rewritten green|red after
# (two writes) so a watcher sees the in-flight state. --activity sets an optional
# short label; it only applies with --step.
pl_cmd_run() {
  local doc="$1"; shift
  local only_step="" activity="" steps base cur ledger_file root
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --step)
        only_step="${2:-}"
        [ -n "$only_step" ] || { echo "plan-ledger: --step requires an id" >&2; return 2; }
        shift 2
        ;;
      --activity)
        activity="${2:-}"
        [ -n "$activity" ] || { echo "plan-ledger: --activity requires a label" >&2; return 2; }
        shift 2
        ;;
      *)
        echo "plan-ledger: unknown run flag: $1" >&2; return 2
        ;;
    esac
  done
  [ -z "$activity" ] || [ -n "$only_step" ] || { echo "plan-ledger: --activity requires --step" >&2; return 2; }

  # Parse first — on failure write NOTHING (crit8).
  steps=$(pl_parse_steps "$doc") || return 2

  base="${PUSH_REVIEW_BASE:-$(detect_base_branch)}"
  root=$(detect_project_root)
  [ -n "$root" ] || { echo "plan-ledger: not in a git repo" >&2; return 2; }
  ledger_file=$(pl_ledger_path) || { echo "plan-ledger: cannot resolve ledger path" >&2; return 2; }

  # cd to project root so checks run there (and diff_sha is repo-relative).
  cd "$root" || { echo "plan-ledger: cannot cd to $root" >&2; return 2; }

  cur=$(pl_diff_sha "$base") || { echo "plan-ledger: git diff ${base}...HEAD failed" >&2; return 2; }

  local branch slug
  branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null) || return 2
  slug=$(branch_slug "$branch")

  # Prior on-disk entries, indexed by id. Read for BOTH full and --step runs: a
  # --step run preserves the other steps' entries verbatim, and EVERY run passes
  # each step's prior entry to the serializer so a prior red is archived into
  # retries[] (and prior retries carry forward). Captured once here from the
  # ORIGINAL ledger, before any write below — so the --step running pre-write
  # cannot overwrite the prior we archive from. Fail-closed: a present-but-corrupt
  # ledger returns 2 (never silently dropped); an absent ledger is the empty map.
  local existing="{}"
  local prior
  if prior=$(pl_read_ledger "$ledger_file" 2>/dev/null); then
    existing=$(jq '[.steps[] | {key: .id, value: .}] | from_entries' <<< "$prior") \
      || { echo "plan-ledger: corrupt prior ledger at $ledger_file" >&2; return 2; }
  elif [ -s "$ledger_file" ]; then
    # File exists and is non-empty but pl_read_ledger refused it (unparseable
    # json): corrupt, not absent. Fail closed rather than silently archiving
    # nothing and clobbering it with a fresh run.
    echo "plan-ledger: corrupt prior ledger at $ledger_file" >&2; return 2
  fi

  # Write #1 (only with --step): mark the target step `running` + started_at
  # (+ activity) and persist BEFORE running its check, so a watcher sees the
  # in-flight state. Other steps keep their prior entry (or seed pending). The
  # green|red rewrite (write #2) happens after the check, below.
  local started_at=""
  if [ -n "$only_step" ]; then
    started_at=$(pl_now)
    local pre_steps
    pre_steps=$(jq -n \
      --argjson ex "$existing" --argjson steps "$steps" \
      --arg only "$only_step" --arg now "$started_at" --arg act "$activity" '
      [ $steps[] | .id as $id
        | ($ex[$id]) as $p
        | ($steps[] | select(.id==$id)) as $s
        | if $id == $only
          then { id: $s.id, title: $s.title, check: $s.check,
                 status: "running",
                 started_at: $now,
                 activity: (if $act == "" then null else $act end),
                 exit_code: null, diff_sha: null,
                 last_run: $now, evidence_tail: null,
                 ac_refs: ($s.ac_refs // []),
                 depends_on: ($s.depends_on // []),
                 input: ($s.input // null),
                 retries: ($p.retries // []) }
          elif $p != null
          then $p
               | .ac_refs    = ($s.ac_refs // [])
               | .depends_on = ($s.depends_on // [])
               | .input      = ($s.input // null)
          else { id: $s.id, title: $s.title, check: $s.check,
                 status: "pending", started_at: null, activity: null,
                 exit_code: null, diff_sha: null,
                 last_run: null, evidence_tail: null,
                 ac_refs: ($s.ac_refs // []),
                 depends_on: ($s.depends_on // []),
                 input: ($s.input // null),
                 retries: [] }
          end
      ]') || { echo "plan-ledger: failed to assemble running pre-write" >&2; return 2; }
    local pre_ledger
    pre_ledger=$(jq -n \
      --arg branch "$branch" --arg base "$base" --arg doc "$doc" \
      --arg now "$started_at" --argjson steps "$pre_steps" '
      { version: 1, branch: $branch, base_branch: $base, plan_doc: $doc,
        updated_at: $now, summary: {}, next: null, steps: $steps }') \
      || { echo "plan-ledger: failed to assemble running pre-ledger" >&2; return 2; }
    pre_ledger=$(pl_recompute "$pre_ledger" "$cur") \
      || { echo "plan-ledger: failed to recompute running pre-ledger" >&2; return 2; }
    pl_write_ledger "$ledger_file" "$pre_ledger" \
      || { echo "plan-ledger: running pre-write failed" >&2; return 2; }
  fi

  # Build the steps array.
  local out_steps id check status code evidence tmpout new_entry
  out_steps="[]"
  while IFS= read -r id; do
    [ -n "$id" ] || continue
    check=$(jq -r --arg id "$id" '.[] | select(.id==$id) | .check' <<< "$steps") \
      || { echo "plan-ledger: failed to read check for step $id" >&2; return 2; }
    if [ -n "$only_step" ] && [ "$id" != "$only_step" ]; then
      # Carry the prior entry forward, but RE-DERIVE the authored fields
      # (ac_refs/depends_on/input) from the current parsed step $s — they live in
      # the plan doc and must reflect any edit since the last run (spec: authored
      # fields are re-derived every run), while all engine state (status,
      # exit_code, diff_sha, last_run, evidence_tail, retries) is preserved. No
      # prior entry -> seed a pending step with the current authored fields.
      new_entry=$(jq -n --argjson ex "$existing" --argjson steps "$steps" --arg id "$id" '
        ($ex[$id]) as $p
        | ($steps[] | select(.id==$id)) as $s
        | if $p != null
          then $p
               | .ac_refs    = ($s.ac_refs // [])
               | .depends_on = ($s.depends_on // [])
               | .input      = ($s.input // null)
          else { id: $s.id, title: $s.title, check: $s.check,
                 status: "pending", started_at: null, activity: null,
                 exit_code: null, diff_sha: null,
                 last_run: null, evidence_tail: null,
                 ac_refs: ($s.ac_refs // []),
                 depends_on: ($s.depends_on // []),
                 input: ($s.input // null),
                 retries: [] }
          end
      ') || { echo "plan-ledger: failed to assemble entry for step $id" >&2; return 2; }
    else
      tmpout="$ledger_file.run.$$.$id"
      mkdir -p "$(dirname "$ledger_file")" 2>/dev/null || true
      bash -c "$check" >"$tmpout" 2>&1
      code=$?
      [ "$code" -eq 0 ] && status="green" || status="red"
      evidence=$(pl_evidence "$(cat "$tmpout")")
      rm -f "$tmpout"
      # Prior on-disk entry for this id (or "null"): the builder archives it into
      # retries[] iff it was red, and carries its prior retries forward.
      local prior_entry
      prior_entry=$(jq -cn --argjson ex "$existing" --arg id "$id" '$ex[$id] // null') \
        || { echo "plan-ledger: failed to read prior entry for step $id" >&2; return 2; }
      new_entry=$(pl_build_step_entry "$steps" "$id" "$status" "$code" "$cur" "$evidence" "" "" "$prior_entry") \
        || { echo "plan-ledger: failed to build entry for step $id" >&2; return 2; }
    fi
    out_steps=$(jq --argjson e "$new_entry" '. + [$e]' <<< "$out_steps") \
      || { echo "plan-ledger: failed to append step $id" >&2; return 2; }
  done < <(jq -r '.[].id' <<< "$steps")

  # Assemble the full ledger, then recompute summary/next against current sha.
  local ledger
  ledger=$(jq -n \
    --arg branch "$branch" \
    --arg base "$base" \
    --arg doc "$doc" \
    --arg now "$(pl_now)" \
    --argjson steps "$out_steps" '
    { version: 1, branch: $branch, base_branch: $base, plan_doc: $doc,
      updated_at: $now,
      summary: {}, next: null, steps: $steps }
  ') || { echo "plan-ledger: failed to assemble ledger" >&2; return 2; }
  ledger=$(pl_recompute "$ledger" "$cur") \
    || { echo "plan-ledger: failed to recompute summary" >&2; return 2; }

  pl_write_ledger "$ledger_file" "$ledger" || { echo "plan-ledger: ledger write failed" >&2; return 2; }

  pl_summary_line "$ledger" "$slug"
  pl_all_fresh "$ledger" && return 0 || return 1
}

# pl_ac_coverage_lines LEDGER_JSON CUR SPEC_DOC
# Print a human-readable AC-coverage report to stdout (REPORT-ONLY — the caller
# must never let it affect exit codes or the gate). For each AC id declared in
# SPEC_DOC (via pl_parse_acs), list the ledger steps whose ac_refs name it and
# whether at least one of those covering steps is fresh-green (status==green AND
# diff_sha==CUR); an AC with no fresh-green covering step is flagged UNCOVERED.
# Spec-less (SPEC_DOC empty / "none" / missing / no AC ids) -> print nothing and
# return 0: coverage is skipped, never a blocker. Never writes, never errors out.
pl_ac_coverage_lines() {
  local ledger="$1" cur="$2" spec="$3" acs
  case "$(printf '%s' "$spec" | tr '[:upper:]' '[:lower:]')" in
    ""|none) return 0 ;;
  esac
  acs=$(pl_parse_acs "$spec")
  [ -n "$acs" ] || return 0
  printf 'AC coverage (report-only):\n'
  # For each AC id, jq finds covering steps and whether any is fresh-green.
  local ac line
  while IFS= read -r ac; do
    [ -n "$ac" ] || continue
    line=$(jq -rn --argjson l "$ledger" --arg cur "$cur" --arg ac "$ac" '
      [ $l.steps[] | select((.ac_refs // []) | index($ac)) ] as $cov
      | ($cov | map(.id)) as $ids
      | ($cov | any(.status == "green" and .diff_sha == $cur)) as $fresh
      | if ($ids | length) == 0
        then "  " + $ac + ": UNCOVERED (no step references it)"
        elif $fresh
        then "  " + $ac + ": covered by " + ($ids | join(", "))
        else "  " + $ac + ": UNCOVERED (" + ($ids | join(", ")) + " not fresh-green)"
        end
    ') || { echo "plan-ledger: failed to compute AC coverage for $ac" >&2; return 0; }
    printf '%s\n' "$line"
  done <<< "$acs"
  return 0
}

# pl_cmd_status
# Read the current branch's ledger (absent -> exit 2), recompute summary/next vs
# the current diff_sha WITHOUT running checks, write the refreshed ledger, print
# the summary line, then the AC-coverage report (report-only). Exit 0 iff all
# fresh-green, else 1 — AC coverage NEVER changes the exit code.
pl_cmd_status() {
  local base cur ledger_file ledger slug branch
  base="${PUSH_REVIEW_BASE:-$(detect_base_branch)}"
  ledger_file=$(pl_ledger_path) || { echo "plan-ledger: cannot resolve ledger path" >&2; return 2; }
  ledger=$(pl_read_ledger "$ledger_file") || { echo "plan-ledger: no ledger at $ledger_file" >&2; return 2; }
  cur=$(pl_diff_sha "$base") || { echo "plan-ledger: git diff ${base}...HEAD failed" >&2; return 2; }

  branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null) || return 2
  slug=$(branch_slug "$branch")

  # Self-heal orphaned `running` steps (crash between the two run writes) back to
  # pending before recomputing, so a stale running can't wedge status/push-gate.
  ledger=$(pl_heal_orphans "$ledger") \
    || { echo "plan-ledger: failed to heal orphaned running steps" >&2; return 2; }
  ledger=$(pl_recompute "$ledger" "$cur") \
    || { echo "plan-ledger: failed to recompute summary" >&2; return 2; }
  pl_write_ledger "$ledger_file" "$ledger" || { echo "plan-ledger: ledger write failed" >&2; return 2; }

  pl_summary_line "$ledger" "$slug"

  # AC-coverage report (report-only): resolve the plan's **Spec:** doc relative to
  # the project root, parse its AC ids, and report which are covered by a
  # fresh-green step. REPORT-ONLY — coverage must never change the status exit
  # code or wedge the gate (spec Non-Goal). A coverage failure is reported to
  # stderr (not silently dropped), then status proceeds with its real exit code.
  local plan_doc spec_field spec_path root
  plan_doc=$(jq -r '.plan_doc // ""' <<< "$ledger") \
    || { echo "plan-ledger: could not read plan_doc for AC coverage (skipping report)" >&2; plan_doc=""; }
  if [ -n "$plan_doc" ]; then
    root=$(detect_project_root)
    [ -f "$plan_doc" ] || { [ -n "$root" ] && [ -f "$root/$plan_doc" ] && plan_doc="$root/$plan_doc"; }
    if [ -f "$plan_doc" ]; then
      spec_field=$(pl_doc_field "$plan_doc" Spec)
      spec_path="$spec_field"
      case "$(printf '%s' "$spec_field" | tr '[:upper:]' '[:lower:]')" in
        ""|none) spec_path="$spec_field" ;;
        *) [ -f "$spec_path" ] || { [ -n "$root" ] && [ -f "$root/$spec_field" ] && spec_path="$root/$spec_field"; } ;;
      esac
      # pl_ac_coverage_lines is itself report-only: it returns 0 even on an
      # internal failure (after emitting a stderr diagnostic), so status's exit
      # code below is decided solely by fresh-green state.
      pl_ac_coverage_lines "$ledger" "$cur" "$spec_path"
    fi
  fi

  pl_all_fresh "$ledger" && return 0 || return 1
}

# pl_self_test
# Parse a tiny inline fixture doc and assert pl_parse_steps yields the expected
# two-step array. Minimal but real. Exit 0/1.
pl_self_test() {
  local dir doc out
  dir=$(mktemp -d) || return 1
  doc="$dir/selftest-plan.md"
  cat > "$doc" <<'EOF'
# Self-test Plan

## Steps (machine-readable)

```json
[
  { "id": "s1", "title": "ok", "check": "true" },
  { "id": "s2", "title": "fail", "check": "false" }
]
```
EOF
  if ! out=$(pl_parse_steps "$doc"); then
    rm -rf "$dir"; echo "plan-ledger --self-test: parse failed" >&2; return 1
  fi
  rm -rf "$dir"
  if [ "$(jq -r 'length' <<< "$out")" != "2" ] \
    || [ "$(jq -r '.[0].id' <<< "$out")" != "s1" ] \
    || [ "$(jq -r '.[1].check' <<< "$out")" != "false" ]; then
    echo "plan-ledger --self-test: unexpected parse result" >&2; return 1
  fi
  echo "plan-ledger --self-test: ok"
  return 0
}

# main "$@"
# CLI dispatch. Requires jq + git. Unknown command -> exit 2.
main() {
  command -v jq  >/dev/null 2>&1 || { echo "plan-ledger: jq is required" >&2; exit 2; }
  command -v git >/dev/null 2>&1 || { echo "plan-ledger: git is required" >&2; exit 2; }

  local cmd="${1:-}"; shift || true
  case "$cmd" in
    run)
      [ -n "${1:-}" ] || { echo "plan-ledger: run requires a plan doc path" >&2; exit 2; }
      pl_cmd_run "$@"; exit $?
      ;;
    status)
      pl_cmd_status; exit $?
      ;;
    preflight)
      pl_cmd_preflight "${1:-}"; exit $?
      ;;
    path)
      pl_ledger_path || { echo "plan-ledger: cannot resolve ledger path" >&2; exit 2; }
      exit 0
      ;;
    root)
      local root
      root=$(detect_project_root)
      [ -n "$root" ] || { echo "plan-ledger: not in a git repo" >&2; exit 2; }
      printf '%s\n' "$root"
      exit 0
      ;;
    --self-test)
      pl_self_test; exit $?
      ;;
    *)
      echo "plan-ledger: usage: run <doc> [--step <id>] [--activity <label>] | status | preflight [<doc>] | path | root | --self-test" >&2
      exit 2
      ;;
  esac
}

# Guarded main: run only when executed directly, not when sourced.
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  main "$@"
fi
