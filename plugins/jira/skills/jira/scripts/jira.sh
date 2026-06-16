#!/usr/bin/env bash
# jira.sh — Jira CLI dispatcher. Strips global flags (--api-version, --lean) from
# anywhere in argv, then routes <family> <action> to the matching lib module.
# Reads creds from JIRA_* env (never .env). See SKILL.md for full usage.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB="$SCRIPT_DIR/lib"
FAMILIES="search issue board sprint worklog project user attachment raw"

usage() {
  cat >&2 <<'EOF'
jira — Jira from the session (Cloud + Server/DC)

Usage: jira [--api-version N] [--lean] <family> <action> [options]

Families:
  search       JQL search
  issue        get|create|update|delete|comment|transition|transitions|assign
  board        list|get|issues
  sprint       list|get|create|issues|move|start|complete
  worklog      add|list|delete
  project      list|get|versions|components
  user         whoami|search|get
  attachment   add|list|get
  raw          <METHOD> <path> [body]

Environment:
  JIRA_BASE_URL                      required (e.g. https://acme.atlassian.net)
  JIRA_PAT                           Bearer auth (Cloud or Server/DC)
  JIRA_EMAIL + JIRA_API_TOKEN        basic auth (Cloud API token)
  JIRA_API_VERSION                   2 or 3 (default 3)
EOF
  exit 1
}

# Strip global flags from anywhere; export resolved transport globals.
args=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --api-version)   export _JIRA_VER="${2:-}"; shift 2;;
    --api-version=*) export _JIRA_VER="${1#*=}"; shift;;
    --lean)          export _JIRA_LEAN=1; shift;;
    *)               args+=("$1"); shift;;
  esac
done
set -- ${args[@]+"${args[@]}"}

[[ $# -lt 1 ]] && usage
family="$1"; shift

case " $FAMILIES " in
  *" $family "*) ;;
  *) echo "jira: unknown family '$family'" >&2; usage;;
esac

# shellcheck source=/dev/null
source "$LIB/http.sh"
source "$LIB/adf.sh"
source "$LIB/paginate.sh"
# shellcheck source=/dev/null
source "$LIB/$family.sh"

jira_require_env
"jira_$family" "$@"
