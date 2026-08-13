#!/usr/bin/env bash
# Check runner for the `plan` family: probe Jira once, then execute each step's
# `check` and record the verdict in the ledger. No `set` (sourced library).
# Source via:   . "${BASH_SOURCE[0]%/*}/plan-run.sh"
# Requires plan-parse.sh + plan-store.sh to be sourced first.
#
# A step is green iff its check exits 0. Because every check is a live Jira REST
# assertion, green means "Jira agrees", not "the agent says so".
#
# Infra failure is NOT a Jira disagreement. A missing credential, an unreachable
# host, or a broken jq pipe would each exit non-zero and land every step `red` --
# flipping dashboard cards to Blocked for a reason that has nothing to do with
# the ticket. So we probe once with `user whoami` and, on failure, abort before
# writing anything: statuses are left exactly as they were.

# jira_plan_cli
# Resolve the jira CLI a check invokes as "$JIRA". JIRA_CLI overrides it, which
# is how the bats sandbox points checks at its stubbed copy instead of the host's.
jira_plan_cli() {
  if [ -n "${JIRA_CLI:-}" ]; then
    printf '%s\n' "$JIRA_CLI"
  elif [ -n "${TOOLU_CONFIG_DIR:-}" ]; then
    printf '%s/jira/jira.sh\n' "$TOOLU_CONFIG_DIR"
  elif [ "${TOOLU_HOST_OVERRIDE:-}" = codex ] || {
    [ -z "${TOOLU_HOST_OVERRIDE:-}" ] && [ -n "${PLUGIN_ROOT:-}" ];
  }; then
    printf '%s/jira/jira.sh\n' "${CODEX_HOME:-$HOME/.codex}"
  else
    printf '%s/jira/jira.sh\n' "${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
  fi
}

# jira_plan_probe CLI
# Confirm Jira is reachable and the credentials work. Zero on success.
jira_plan_probe() {
  "$1" user whoami >/dev/null 2>&1
}

# jira_plan_exec_check CLI CHECK ROOT
# Run CHECK with $JIRA bound and cwd at ROOT. Prints combined output; returns the
# check's exit code. Never aborts the caller under `set -e`.
jira_plan_exec_check() {
  local cli="$1" check="$2" root="$3" out code
  if out=$(cd "$root" && JIRA="$cli" bash -c "$check" 2>&1); then code=0; else code=$?; fi
  printf '%s' "$out"
  return "$code"
}

# jira_plan_run KEY DOC [--step ID] [--activity LABEL]
# Merge the doc's steps over the persisted ledger, probe Jira, then run either
# one step (--step) or all of them, writing the ledger after each transition so
# the dashboard sees `running` while a check is in flight.
# Returns non-zero if any executed step ended red.
jira_plan_run() {
  local key="$1" doc="$2"; shift 2
  local only="" activity="" cli root state steps prev ledger targets id check out code failed=0

  while [ $# -gt 0 ]; do
    case "$1" in
      --step)     only="${2:-}"; shift 2 ;;
      --activity) activity="${2:-}"; shift 2 ;;
      *) echo "jira plan run: unknown option '$1'" >&2; return 1 ;;
    esac
  done

  steps=$(jira_plan_parse_steps "$doc") || return 1
  root=$(jira_plan_repo_root)
  state=$(jira_plan_ledger_path "$key") || return 1
  prev=$(jira_plan_read "$state" 2>/dev/null) || prev="null"
  ledger=$(jira_plan_build "$key" "$doc" "$steps" "$prev") || return 1

  if [ -n "$only" ] && ! jq -e --arg id "$only" 'any(.steps[]; .id == $id)' <<< "$ledger" >/dev/null; then
    echo "jira plan run: no step '$only' in $doc" >&2
    return 1
  fi

  cli=$(jira_plan_cli)
  if [ ! -x "$cli" ] && [ ! -f "$cli" ]; then
    echo "jira plan run: jira CLI not found at $cli (set JIRA_CLI)" >&2
    return 1
  fi
  # Probe BEFORE the first write: an unreachable Jira must not manufacture reds.
  if ! jira_plan_probe "$cli"; then
    echo "jira plan run: cannot reach Jira (user whoami failed) — no step statuses were written" >&2
    return 1
  fi

  if [ -n "$only" ]; then targets="$only"; else targets=$(jq -r '.steps[].id' <<< "$ledger"); fi

  while IFS= read -r id; do
    [ -n "$id" ] || continue
    check=$(jq -r --arg id "$id" '.steps[] | select(.id==$id) | .check' <<< "$ledger")

    # First write: running + started_at (+ activity), so the dashboard shows it in flight.
    ledger=$(jira_plan_set_step "$ledger" "$id" "$(jq -n --arg now "$(jira_plan_now)" --arg a "$activity" \
      '{status:"running", started_at:$now, exit_code:null, evidence_tail:null}
       + (if $a == "" then {} else {activity:$a} end)')") || return 1
    jira_plan_write "$state" "$ledger" || return 1

    if out=$(jira_plan_exec_check "$cli" "$check" "$root"); then code=0; else code=$?; fi

    # Second write: the verdict.
    ledger=$(jira_plan_set_step "$ledger" "$id" "$(jq -n \
      --arg st "$([ "$code" -eq 0 ] && echo green || echo red)" \
      --argjson ec "$code" \
      --arg now "$(jira_plan_now)" \
      --argjson ev "$(jira_plan_evidence "$out")" \
      '{status:$st, exit_code:$ec, last_run:$now, evidence_tail:$ev, started_at:null}')") || return 1
    jira_plan_write "$state" "$ledger" || return 1

    if [ "$code" -eq 0 ]; then echo "green  $id"; else echo "red    $id (exit $code)"; failed=1; fi
  done <<< "$targets"

  return "$failed"
}
