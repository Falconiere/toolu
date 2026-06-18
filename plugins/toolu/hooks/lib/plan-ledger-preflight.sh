#!/usr/bin/env bash
# Plan-ledger preflight precondition gate. Sourced by plan-ledger.sh (keeps that
# file under its 500-line size budget). Pure-ish: reads the plan + spec docs and
# the ledger; runs no checks and writes nothing. jq-only via the parse lib.
#
# Depends on (already sourced by plan-ledger.sh before this file): pl_doc_field,
# pl_ledger_path, pl_read_ledger (plan-ledger-parse.sh) + detect_project_root
# (detect.sh).
#
# Source via:  . "${BASH_SOURCE%/*}/plan-ledger-preflight.sh"

# pl_status_is_approved STATUS  ->  return 0 iff STATUS == "approved" (case-insensitive).
pl_status_is_approved() {
  local s
  s=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  [ "$s" = "approved" ]
}

# Default age (seconds) after which a `running` step is considered orphaned (a
# crash between the two writes wedged it). Overridable for tests.
: "${PL_STUCK_THRESHOLD:=300}"

# pl_heal_orphans LEDGER_JSON
# Reset any `running` step whose started_at is older than PL_STUCK_THRESHOLD
# seconds (or has no/empty started_at) back to `pending`, clearing started_at +
# activity. A still-fresh running step is left as-is. Prints the (possibly
# unchanged) ledger json on stdout. Pure: no I/O, no recompute.
#
# Comparison is lexical on the fixed ISO-8601 UTC format (`YYYY-MM-DDTHH:MM:SSZ`
# sorts chronologically). The cutoff = now - threshold is computed in bash via
# epoch math + reformat — portable across BSD (`date -r`) and GNU (`date -d @`),
# and immune to jq mktime's local-timezone interpretation.
pl_heal_orphans() {
  local ledger="$1" now_epoch cutoff_epoch cutoff
  now_epoch=$(date -u +%s)
  cutoff_epoch=$((now_epoch - PL_STUCK_THRESHOLD))
  cutoff=$(date -u -r "$cutoff_epoch" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null \
        || date -u -d "@$cutoff_epoch" +%Y-%m-%dT%H:%M:%SZ) \
    || return 1
  jq --arg cutoff "$cutoff" '
    .steps |= map(
      if .status == "running"
      then
        (.started_at // "") as $st
        | if ($st == "") or ($st < $cutoff)
          then .status = "pending" | .started_at = null | .activity = null
          else . end
      else . end
    )
  ' <<< "$ledger"
}

# pl_cmd_preflight [PLAN_DOC]
# Resolve the plan doc (arg, else the current branch ledger's plan_doc). Read its
# **Status:** and **Spec:** via pl_doc_field. Pass (exit 0, silent) only when the
# plan is Approved AND (spec-less OR the declared spec is Approved).
#
# Exit codes:
#   0  preconditions met.
#   1  preconditions unmet (plan not approved / declared spec unreadable / spec
#      not approved / header-less plan with empty Status). One blocker line each.
#   2  plan doc missing / unreadable, or cannot resolve a plan to check.
pl_cmd_preflight() {
  local plan="${1:-}" root ledger ledger_file

  root=$(detect_project_root)

  # Read the current branch's ledger if present, to (a) resolve the plan_doc when
  # no arg was given and (b) self-heal orphaned `running` steps so a wedged step
  # can't outlive a crash. Healing rewrites the ledger; absence is not an error.
  if ledger_file=$(pl_ledger_path 2>/dev/null) && ledger=$(pl_read_ledger "$ledger_file" 2>/dev/null); then
    local healed
    if healed=$(pl_heal_orphans "$ledger" 2>/dev/null) && [ "$healed" != "$ledger" ]; then
      pl_write_ledger "$ledger_file" "$healed" 2>/dev/null || true
    fi
    if [ -z "$plan" ]; then
      plan=$(jq -r '.plan_doc // ""' <<< "$ledger" 2>/dev/null)
    fi
  fi
  if [ -z "$plan" ]; then
    echo "preflight: no plan doc given and no ledger plan_doc to resolve" >&2
    return 2
  fi

  # A relative plan path is resolved against repo root (matching how it's run).
  local plan_abs="$plan"
  case "$plan" in
    /*) : ;;
    *)  [ -n "$root" ] && plan_abs="$root/$plan" ;;
  esac
  if [ ! -r "$plan_abs" ]; then
    echo "preflight: plan doc not found or unreadable: $plan" >&2
    return 2
  fi

  # Plan status. A header-less/malformed plan yields an empty Status -> blocker
  # (exit 1, not 2 — the doc exists, it just isn't stamped).
  local plan_status
  plan_status=$(pl_doc_field "$plan_abs" Status)
  if [ -z "$plan_status" ]; then
    echo "preflight: plan has no **Status:** header ($plan) — run /toolu:plan-review to stamp it" >&2
    return 1
  fi
  if ! pl_status_is_approved "$plan_status"; then
    echo "preflight: plan not approved (Status: $plan_status) — run /toolu:plan-review" >&2
    return 1
  fi

  # Spec gate. A missing **Spec:** line OR "none" is spec-less -> skip.
  local spec
  spec=$(pl_doc_field "$plan_abs" Spec)
  local spec_lc
  spec_lc=$(printf '%s' "$spec" | tr '[:upper:]' '[:lower:]')
  if [ -z "$spec" ] || [ "$spec_lc" = "none" ]; then
    return 0
  fi

  # Declared spec: resolve relative to repo root, must be readable.
  local spec_abs="$spec"
  case "$spec" in
    /*) : ;;
    *)  [ -n "$root" ] && spec_abs="$root/$spec" ;;
  esac
  if [ ! -r "$spec_abs" ]; then
    echo "preflight: declared spec not found or unreadable: $spec" >&2
    return 1
  fi

  local spec_status
  spec_status=$(pl_doc_field "$spec_abs" Status)
  if ! pl_status_is_approved "$spec_status"; then
    echo "preflight: spec $spec not approved (Status: ${spec_status:-none}) — run /toolu:spec-review" >&2
    return 1
  fi

  return 0
}
