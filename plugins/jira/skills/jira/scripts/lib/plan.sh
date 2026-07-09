#!/usr/bin/env bash
# `plan` family: scaffold a plan doc for a ticket, run its checks, and keep a
# dashboard-readable ledger at <repo>/.claude/tmp/plan-ledger/jira-<KEY>.json.
#
# jira.sh sources only lib/<family>.sh, so this file pulls in its own deps —
# including issue.sh, because `plan init` titles the doc from a live `issue get`.
# Sourcing is relative to THIS file, not $LIB: $LIB is set by jira.sh alone, so a
# test that sources plan.sh directly would otherwise expand to "/plan-parse.sh".
_jira_plan_lib="${BASH_SOURCE[0]%/*}"
# shellcheck source=/dev/null
. "$_jira_plan_lib/plan-parse.sh"
# shellcheck source=/dev/null
. "$_jira_plan_lib/plan-store.sh"
# shellcheck source=/dev/null
. "$_jira_plan_lib/plan-run.sh"
# shellcheck source=/dev/null
. "$_jira_plan_lib/issue.sh"

_jira_plan_usage() {
  cat >&2 <<'EOF'
jira plan — decompose ticket work into verifiable steps the dashboard renders

  plan init <KEY>                                 scaffold the plan doc for a ticket
  plan run <DOC> [--step <id>] [--activity <s>]   run checks, write the ledger
  plan status <KEY>                               print the ledger summary
  plan path <KEY>                                 print the ledger path

Every step's `check` must be a live Jira assertion that exits 0 only when Jira
itself reflects the change.
EOF
  return 1
}

# jira_plan_init KEY
# Write <repo>/.claude/tmp/jira/plans/<KEY>.md, titled from a live `issue get`.
# Refuses to clobber an existing doc.
jira_plan_init() {
  local key="${1:-}" doc summary
  [ -n "$key" ] || { echo "jira plan init: needs an issue key" >&2; return 1; }
  doc=$(jira_plan_doc_path "$key") || return 1
  if [ -e "$doc" ]; then
    echo "jira plan init: $doc already exists" >&2
    return 1
  fi
  summary=$(jira_issue get "$key" | jq -r '.fields.summary // empty') || return 1
  [ -n "$summary" ] || summary="$key"
  mkdir -p "$(dirname "$doc")" || return 1
  cat > "$doc" <<EOF
# $key — $summary

**Date:** $(date -u +%Y-%m-%d)   **Issue:** $key   **Topic:** $summary

## Steps (machine-readable)

\`\`\`json
[]
\`\`\`

Fill the array with {"id","title","check"} objects. A \`check\` is a shell command
that exits 0 **only when Jira itself reflects the change** — assert against a live
read, e.g.

    "\$JIRA" issue get $key --lean | jq -e '.status=="Done"' >/dev/null

\`\$JIRA\` is bound to the jira CLI when the check runs. Then: jira.sh plan run $doc
EOF
  printf '%s\n' "$doc"
}

# jira_plan_status KEY — print the ledger summary and each step's status.
jira_plan_status() {
  local key="${1:-}" state ledger
  [ -n "$key" ] || { echo "jira plan status: needs an issue key" >&2; return 1; }
  state=$(jira_plan_ledger_path "$key") || return 1
  if ! ledger=$(jira_plan_read "$state"); then
    echo "jira plan status: no ledger for $key (run: jira.sh plan run <doc>)" >&2
    return 1
  fi
  jq -r '
    "\(.branch)  \(.summary.green)/\(.summary.total) green   next: \(.next // "-")",
    (.steps[] | "  \(.status)\t\(.id)\t\(.title)")
  ' <<< "$ledger"
}

# jira_plan_path KEY — print the ledger path. Needs no credentials.
jira_plan_path() {
  local key="${1:-}"
  [ -n "$key" ] || { echo "jira plan path: needs an issue key" >&2; return 1; }
  jira_plan_ledger_path "$key"
}

jira_plan() {
  local action="${1:-}" doc key
  [ $# -gt 0 ] && shift
  case "$action" in
    init)   jira_plan_init "$@" ;;
    status) jira_plan_status "$@" ;;
    path)   jira_plan_path "$@" ;;
    run)
      doc="${1:-}"
      [ -n "$doc" ] || { echo "jira plan run: needs a plan doc path" >&2; return 1; }
      shift
      key=$(jira_plan_issue_key "$doc") || return 1
      jira_plan_run "$key" "$doc" "$@"
      ;;
    *) _jira_plan_usage ;;
  esac
}
