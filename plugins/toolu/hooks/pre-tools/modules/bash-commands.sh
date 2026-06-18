#!/usr/bin/env bash
# Pre-tool check: Bash/Shell command validation
# Data-driven: loads allow/deny rules from $TOOLU_SETTINGS_DIR/bash-{allow,deny}list.txt.
# Argv-aware: tokenizes the command and matches "<bin> <flag>" deny rules so
# substring evasion is harder (e.g. `node -e "evil"` is rejected even if `node`
# alone is allowed).
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload on stdin

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"

[[ "$tool_name" != "Bash" && "$tool_name" != "Shell" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)

# read_list + strip_heredocs are sourced from lib/detect.sh.

# bash_tokenize <command>
# Tokenize a shell command into argv tokens, respecting single and double quotes.
# Echoes one token per line.
bash_tokenize() {
  local cmd="$1"
  python3 - "$cmd" <<'PY' 2>/dev/null || printf '%s\n' "$cmd"
import shlex, sys
try:
    for tok in shlex.split(sys.argv[1], posix=True):
        print(tok)
except ValueError:
    # Unparseable (unbalanced quote etc) — print whole string so substring rules still fire.
    print(sys.argv[1])
PY
}

# tokens_contain_rule <rule> <token...>
# Returns 0 if `tokens[0]` matches the first rule token AND every subsequent
# rule token appears as a distinct later argv token. Substring matching is
# never used here — that was the source of false-positives on commit messages
# like `git commit -m "fix cargo test failure"`.
tokens_contain_rule() {
  local rule="$1"; shift
  local -a tokens=("$@")
  local -a rule_tokens
  # shellcheck disable=SC2206  # intentional word-split on whitespace
  rule_tokens=($rule)
  [ ${#rule_tokens[@]} -eq 0 ] && return 1
  [ "${tokens[0]:-}" = "${rule_tokens[0]}" ] || return 1
  local i j want found
  for ((i=1; i<${#rule_tokens[@]}; i++)); do
    want="${rule_tokens[$i]}"
    found=0
    for ((j=1; j<${#tokens[@]}; j++)); do
      if [ "${tokens[$j]}" = "$want" ]; then
        found=1
        break
      fi
    done
    [ "$found" -eq 0 ] && return 1
  done
  return 0
}

# matches_deny <command> <denylist>
# Returns 0 (match) if any deny rule fires against the command, else 1.
# Rules containing whitespace (e.g. `node -e`, `cargo test`) use argv-aware
# multi-token matching only (no substring fallback); single-token rules
# (bare names like `biome`) substring-match anywhere on the command.
matches_deny() {
  local cmd="$1"
  local denylist="$2"
  local rule tokens _tok
  tokens=(); while IFS= read -r _tok; do tokens+=("$_tok"); done < <(bash_tokenize "$cmd")
  # Tokenizer yielded nothing (empty/whitespace-only command): fall back to
  # the raw command as a single token — same shape bash_tokenize itself emits
  # on a parse failure. Substring rules below never consult tokens, so this
  # cannot bypass them.
  [ ${#tokens[@]} -eq 0 ] && tokens=("$cmd")
  while IFS= read -r rule; do
    [ -z "$rule" ] && continue
    if [[ "$rule" == *" "* ]]; then
      if tokens_contain_rule "$rule" "${tokens[@]}"; then
        echo "$rule"
        return 0
      fi
    else
      # Single-token rule: substring match against the command. Bare-name
      # rules (`biome`) land here; rules with internal whitespace
      # (`cargo test`) take the multi-token argv branch above.
      if [[ "$cmd" == *"$rule"* ]]; then
        echo "$rule"
        return 0
      fi
    fi
  done <<< "$denylist"
  return 1
}

# matches_allow <command> <allowlist>
# Returns 0 if any allow rule matches. Multi-token rules are argv-aware
# (same shape as deny); single-token rules substring-match.
matches_allow() {
  local cmd="$1"
  local allowlist="$2"
  local rule tokens _tok
  tokens=(); while IFS= read -r _tok; do tokens+=("$_tok"); done < <(bash_tokenize "$cmd")
  # Same empty-tokenization fallback as matches_deny.
  [ ${#tokens[@]} -eq 0 ] && tokens=("$cmd")
  while IFS= read -r rule; do
    [ -z "$rule" ] && continue
    if [[ "$rule" == *" "* ]]; then
      if tokens_contain_rule "$rule" "${tokens[@]}"; then
        return 0
      fi
    else
      if [[ "$cmd" == *"$rule"* ]]; then
        return 0
      fi
    fi
  done <<< "$allowlist"
  return 1
}

# Public entry — invokable when the script is sourced (used by tests).
#
# Semantics: deny is evaluated FIRST so a deny result still carries the rule
# that fired. The allowlist is an explicit override on top of deny, intended
# for project-specific exemptions: deny + matching allow → allow; deny with
# no allow match → deny; no deny match → allow (default-open).
bash_commands_decide() {
  local cmd="$1"
  local allowlist denylist hit
  allowlist=$(read_list "$TOOLU_SETTINGS_DIR/bash-allowlist.txt")
  denylist=$(read_list "$TOOLU_SETTINGS_DIR/bash-denylist.txt")

  # Strip heredoc bodies so we only check actual commands, not prose in commit messages.
  cmd=$(printf '%s\n' "$cmd" | strip_heredocs)

  if hit=$(matches_deny "$cmd" "$denylist"); then
    if matches_allow "$cmd" "$allowlist"; then
      echo "allow"
    else
      echo "deny:$hit"
    fi
    return 0
  fi

  echo "allow"
  return 0
}

# When sourced (tests), return early.
(return 0 2>/dev/null) && return 0

command=$(echo "$input" | jq -r '.tool_input.command // ""')
decision=$(bash_commands_decide "$command")

case "$decision" in
  deny:*)
    rule="${decision#deny:}"
    jq -n --arg rule "$rule" '{
      "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": ("Command blocked by deny rule: " + $rule)
      }
    }'
    ;;
esac

exit 0
