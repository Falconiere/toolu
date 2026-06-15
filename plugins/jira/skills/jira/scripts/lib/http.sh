#!/usr/bin/env bash
# http.sh — shared Jira transport: credential resolution, the curl wrapper, lean
# output. Credentials come from JIRA_* env, or are reused from an installed
# ankitpokhrel jira-cli (config + OS keyring) when the env vars are unset.
# Sourced by jira.sh and every per-family lib module; defines functions only
# (no top-level side effects), so it is safe to source in unit tests.

# jira_require_env: validate JIRA_* env and resolve transport state.
# Sets _JIRA_AUTH_MODE (bearer|basic), _JIRA_VER (2|3), _JIRA_BASE (no trailing
# slash). Returns 1 (naming the offending var) on missing tools/creds/base/version.
jira_require_env() {
  command -v curl >/dev/null 2>&1 || { echo "jira: curl required" >&2; return 1; }
  command -v jq   >/dev/null 2>&1 || { echo "jira: jq required" >&2; return 1; }

  # Fill any unset JIRA_* from an installed jira CLI (best-effort; explicit env
  # always wins, and a missing CLI is a no-op — never an error).
  jira_load_cli_creds

  if [[ -z "${JIRA_BASE_URL:-}" ]]; then jira_creds_help; return 1; fi
  _JIRA_BASE="$JIRA_BASE_URL"
  while [[ "$_JIRA_BASE" == */ ]]; do _JIRA_BASE="${_JIRA_BASE%/}"; done

  if [[ -n "${JIRA_PAT:-}" ]]; then
    _JIRA_AUTH_MODE="bearer"
  elif [[ -n "${JIRA_EMAIL:-}" && -n "${JIRA_API_TOKEN:-}" ]]; then
    _JIRA_AUTH_MODE="basic"
  else
    jira_creds_help; return 1
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

# jira_download PATH OUTFILE: authenticated GET that follows redirects and
# writes the response body to OUTFILE (Jira attachment content 302-redirects to
# a media host). Auth is inlined rather than shared via a nameref to stay
# portable to bash 3.2.
jira_download() {
  local path="$1" out="$2"
  local args=(-sS --fail-with-body -L -o "$out" -H "Accept: */*")
  if [[ "${_JIRA_AUTH_MODE:-}" == "bearer" ]]; then
    args+=(-H "Authorization: Bearer ${JIRA_PAT}")
  else
    args+=(-u "${JIRA_EMAIL}:${JIRA_API_TOKEN}")
  fi
  curl "${args[@]}" "${_JIRA_BASE}${path}"
}

# ── credential discovery (ankitpokhrel jira-cli) ───────────────────────────
# jira-cli keeps server+login in a flat YAML config and the API token in one of
# several places. We reuse them to fill any unset JIRA_* var, matching jira-cli's
# own token order (config api_token → $JIRA_API_TOKEN → ~/.netrc → OS keyring),
# so a configured CLI needs no extra setup. Explicit JIRA_* env always wins.

# _jira_yaml_get FILE KEY: print a top-level scalar (key: value) from flat YAML.
_jira_yaml_get() {
  [[ -r "$1" ]] || return 0
  awk -v k="$2" 'index($0, k":")==1 { sub(/^[^:]*:[[:space:]]*/,""); sub(/[[:space:]]+$/,""); print; exit }' "$1"
}

# _jira_keyring_token LOGIN: best-effort token from the OS keyring (macOS
# security, then Linux secret-tool). Empty string on any miss — never errors.
# macOS may require interactive keychain authorization the first time.
_jira_keyring_token() {
  local login="$1" svc="${JIRA_KEYRING_SERVICE:-jira-cli}" tok=""
  if command -v security >/dev/null 2>&1; then
    tok=$(security find-generic-password -s "$svc" -a "$login" -w 2>/dev/null || true)
  fi
  if [[ -z "$tok" ]] && command -v secret-tool >/dev/null 2>&1; then
    tok=$(secret-tool lookup service "$svc" username "$login" 2>/dev/null || true)
  fi
  printf '%s' "$tok"
}

# _jira_netrc_token HOST: token (password) for HOST from ~/.netrc (or $NETRC).
# Flattens tokens so multi-line entries work. Empty on miss — never errors.
_jira_netrc_token() {
  local host="$1" file="${NETRC:-$HOME/.netrc}"
  [[ -r "$file" ]] || return 0
  awk -v h="$host" '
    { for (i=1;i<=NF;i++) a[++n]=$i }
    END { for (i=1;i<=n;i++) {
            if (a[i]=="machine" && a[i+1]==h) inhost=1
            else if (a[i]=="machine") inhost=0
            if (inhost && a[i]=="password") { print a[i+1]; exit }
          } }' "$file"
}

# jira_load_cli_creds: fill unset JIRA_* from the jira-cli config + keyring.
# No-op (and no error) when the CLI is not configured.
jira_load_cli_creds() {
  local cfg="${JIRA_CLI_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/.jira/.config.yml}"
  [[ -r "$cfg" ]] || return 0
  local server login installation
  server=$(_jira_yaml_get "$cfg" server)
  login=$(_jira_yaml_get "$cfg" login)
  installation=$(_jira_yaml_get "$cfg" installation)
  [[ -z "${JIRA_BASE_URL:-}" && -n "$server" ]] && export JIRA_BASE_URL="$server"
  [[ -z "${JIRA_EMAIL:-}"    && -n "$login"  ]] && export JIRA_EMAIL="$login"
  if [[ -z "${JIRA_API_VERSION:-}" && -n "$installation" ]]; then
    case "$installation" in
      [Cc]loud) export JIRA_API_VERSION=3 ;;
      *)        export JIRA_API_VERSION=2 ;;
    esac
  fi
  if [[ -z "${JIRA_PAT:-}" && -z "${JIRA_API_TOKEN:-}" ]]; then
    # Match jira-cli's token sources, in its order: config api_token, then
    # ~/.netrc (by server host), then the OS keyring.
    local tok host
    tok=$(_jira_yaml_get "$cfg" api_token)
    if [[ -z "$tok" && -n "$server" ]]; then
      host="${server#*://}"; host="${host%%/*}"
      tok=$(_jira_netrc_token "$host")
    fi
    [[ -z "$tok" && -n "$login" ]] && tok=$(_jira_keyring_token "$login")
    [[ -n "$tok" ]] && export JIRA_API_TOKEN="$tok"
  fi
  return 0
}

# jira_creds_help: friendly, non-error guidance when no credentials resolve.
# Reads as a one-time setup step, not a failure.
jira_creds_help() {
  cat >&2 <<'EOF'
jira: no Jira credentials found yet — let's get you connected.

This plugin reuses your `jira` CLI login automatically when it's configured,
or reads JIRA_* environment variables. Right now neither is set.

Connect with either:

  • Your jira CLI (easiest if it's installed):
      jira init
    then re-run your command — the plugin picks up the server, login, and
    API token from the CLI automatically.

  • Or environment variables:
      export JIRA_BASE_URL=https://your-site.atlassian.net
      export JIRA_EMAIL=you@example.com
      export JIRA_API_TOKEN=…        # id.atlassian.com → Security → API tokens
    (Jira Server/Data Center: use JIRA_PAT instead of JIRA_EMAIL + JIRA_API_TOKEN.)

Nothing is broken — this is just a one-time setup step.
EOF
}
