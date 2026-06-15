#!/usr/bin/env bash
# http.sh — shared Jira transport: env/auth resolution, the curl wrapper, lean
# output. Sourced by jira.sh and every per-family lib module; defines functions
# only (no top-level side effects), so it is safe to source in unit tests.

# jira_require_env: validate JIRA_* env and resolve transport state.
# Sets _JIRA_AUTH_MODE (bearer|basic), _JIRA_VER (2|3), _JIRA_BASE (no trailing
# slash). Returns 1 (naming the offending var) on missing tools/creds/base/version.
jira_require_env() {
  command -v curl >/dev/null 2>&1 || { echo "jira: curl required" >&2; return 1; }
  command -v jq   >/dev/null 2>&1 || { echo "jira: jq required" >&2; return 1; }

  if [[ -z "${JIRA_BASE_URL:-}" ]]; then
    echo "jira: JIRA_BASE_URL unset" >&2
    return 1
  fi
  _JIRA_BASE="$JIRA_BASE_URL"
  while [[ "$_JIRA_BASE" == */ ]]; do _JIRA_BASE="${_JIRA_BASE%/}"; done

  if [[ -n "${JIRA_PAT:-}" ]]; then
    _JIRA_AUTH_MODE="bearer"
  elif [[ -n "${JIRA_EMAIL:-}" && -n "${JIRA_API_TOKEN:-}" ]]; then
    _JIRA_AUTH_MODE="basic"
  else
    echo "jira: no credentials — set JIRA_PAT, or JIRA_EMAIL + JIRA_API_TOKEN" >&2
    return 1
  fi

  _JIRA_VER="${_JIRA_VER:-${JIRA_API_VERSION:-3}}"
  if [[ "$_JIRA_VER" != "2" && "$_JIRA_VER" != "3" ]]; then
    echo "jira: api version must be 2 or 3 (got '$_JIRA_VER')" >&2
    return 1
  fi
  export _JIRA_AUTH_MODE _JIRA_VER _JIRA_BASE
}

# jira_curl METHOD PATH [BODY]: authenticated request to _JIRA_BASE+PATH.
# Adds the resolved auth, JSON Accept, and --fail-with-body; BODY (when given)
# is sent as a JSON payload. Stdout is the raw response body.
jira_curl() {
  local method="$1" path="$2" body="${3:-}"
  local args=(-sS --fail-with-body -X "$method" -H "Accept: application/json")
  if [[ "${_JIRA_AUTH_MODE:-}" == "bearer" ]]; then
    args+=(-H "Authorization: Bearer ${JIRA_PAT}")
  else
    args+=(-u "${JIRA_EMAIL}:${JIRA_API_TOKEN}")
  fi
  if [[ -n "$body" ]]; then
    args+=(-H "Content-Type: application/json" --data "$body")
  fi
  curl "${args[@]}" "${_JIRA_BASE}${path}"
}

# jira_lean [JQ_PROJECTION]: filter stdin through PROJECTION when _JIRA_LEAN=1
# (token-frugal output for LLM prompts), else pretty-print the full body.
jira_lean() {
  local projection="${1:-.}"
  if [[ "${_JIRA_LEAN:-}" == "1" ]]; then
    jq "$projection"
  else
    jq '.'
  fi
}

# jira_urlencode TEXT: percent-encode TEXT for safe use as a URL query value
# (spaces, =, &, etc.). Used by families that pass user input via query params.
jira_urlencode() {
  jq -nr --arg s "$1" '$s|@uri'
}
