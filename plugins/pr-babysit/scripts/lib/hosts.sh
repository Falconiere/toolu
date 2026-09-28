#!/usr/bin/env bash
# hosts.sh — fixer hosts: how each agent CLI is started under `herdr agent
# start`, the default tier -> model/effort table, and the usage-limit pattern.
#
# Sourced after common.sh. A bash 3.2 port of epic-orchestrator's hosts.ts
# (argv) and route.ts (DEFAULT_TABLE); __tests__/hosts-parity.bats evaluates
# those TypeScript originals live and fails on any drift. Babysit fixers run
# on claude, codex and cursor only (OpenCode is out of scope).

# Characters herdr can type into the pane's interactive shell unquoted.
PB_SHELL_SAFE_RE='^[A-Za-z0-9_./:=,@%+#-]+$'

# pb_host_kind_try NAME -> canonical host (claude|codex|cursor), or return 1.
pb_host_kind_try() {
  local name
  name=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
  case "$name" in
    claude|claude-code) echo claude ;;
    codex) echo codex ;;
    cursor|cursor-agent) echo cursor ;;
    *) return 1 ;;
  esac
}

# pb_host_kind NAME -> canonical host, or exit config_invalid.
pb_host_kind() {
  pb_host_kind_try "$1" && return 0
  pb_fail config_invalid "unknown host '$1'; use one of claude, codex, cursor" "$(jq -nc --arg h "$1" '{host:$h}')"
}

# pb_host_cli KIND -> the executable herdr starts for that host.
pb_host_cli() {
  case "$1" in
    claude) echo claude ;;
    codex) echo codex ;;
    cursor) echo cursor-agent ;;
    *) return 1 ;;
  esac
}

# pb_agent_args KIND NAME MODEL EFFORT BYPASS PERMISSION_MODE
# Print the agent CLI args, one per line (none contains a newline: every arg
# must match PB_SHELL_SAFE_RE, or this exits config_invalid). Empty MODEL or
# EFFORT means "host default". BYPASS is true (unattended) or false (safe).
pb_agent_args() {
  local kind="$1" name="$2" model="$3" effort="$4" bypass="$5" mode="$6" args="" a
  case "$kind:$bypass" in
    claude:true)  args="--dangerously-skip-permissions" ;;
    claude:false) args="--permission-mode
$mode" ;;
    codex:true)   args="--dangerously-bypass-approvals-and-sandbox" ;;
    codex:false)  args="--ask-for-approval
on-request
--sandbox
workspace-write" ;;
    cursor:true)  args="--yolo
--trust
--approve-mcps" ;;
    cursor:false) args="--trust" ;;
    *) pb_fail config_invalid "no argv for host '$kind' (bypass=$bypass)" ;;
  esac
  [ "$kind" = claude ] && args="$args
-n
$name"
  [ -n "$model" ] && args="$args
--model
$model"
  if [ -n "$effort" ]; then
    case "$kind" in
      claude) args="$args
--effort
$effort" ;;
      # Unquoted TOML: codex takes the raw string literally.
      codex) args="$args
-c
model_reasoning_effort=$effort" ;;
      # Cursor encodes effort in the model id; the table picks the id.
      cursor) ;;
    esac
  fi
  while IFS= read -r a; do
    [[ "$a" =~ $PB_SHELL_SAFE_RE ]] && continue
    pb_fail config_invalid "unsafe $kind arg for the pane shell: $a" "$(jq -nc --arg a "$a" '{arg:$a}')"
  done <<<"$args"
  printf '%s\n' "$args"
}

# pb_default_routing_json -> {host: [4 × {model?, effort?}]}, tiers trivial,
# standard, complex, critical. Effort rises with the tier; the model steps up
# where a host has a stronger one. Mirrors epic route.ts DEFAULT_TABLE.
pb_default_routing_json() {
  cat <<'EOF'
{"claude":[{"model":"sonnet","effort":"low"},{"model":"sonnet","effort":"medium"},{"model":"opus","effort":"high"},{"model":"opus","effort":"xhigh"}],
 "codex":[{"model":"gpt-6-sol","effort":"low"},{"model":"gpt-6-sol","effort":"medium"},{"model":"gpt-6-sol","effort":"high"},{"model":"gpt-6-sol","effort":"xhigh"}],
 "cursor":[{"model":"composer-2.5"},{"model":"gpt-5.6-sol-high"},{"model":"claude-opus-5-thinking-high"},{"model":"gpt-5.6-sol-xhigh"}]}
EOF
}

# pb_host_limit_re -> usage/rate-limit messages the agent CLIs print when the
# provider throttles them (Oniguruma syntax for jq `test(re; "i")`). Matched
# only against the last lines of a settled fixer pane.
pb_host_limit_re() {
  printf '%s' 'usage limit|rate[- ]limit(ed)?\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\b429\b|limit will reset|try again (at|in) '
}

# pb_host_limited TEXT -> 0 when TEXT carries a usage-limit message.
pb_host_limited() {
  jq -e -n --arg t "$1" --arg re "$(pb_host_limit_re)" '$t | test($re; "i")' >/dev/null
}
