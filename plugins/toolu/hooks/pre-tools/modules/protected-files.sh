#!/usr/bin/env bash
# Pre-tool check: Protect quality infrastructure files from edits.
# Data-driven: globs come from $TOOLU_SETTINGS_DIR/protected-files.txt.
#
# Delivery is a MODE (gates.protectedFiles.mode), not a constant. It ships as
# `ask`: the user is prompted with a loud warning and decides, per call. It
# used to be an unconditional deny with no way through in-session, which is
# what github.com/Falconiere/toolu/issues/176 reported — an agent could be told
# "yes, edit that .env.example" by the user and STILL be refused, with the deny
# text claiming an override existed that did not.
#
# `block` restores the old hard deny; `advise` only warns; `off` disables the
# check. On a host that cannot prompt, `ask` degrades to `block`, never to
# `advise` — see lib/gate-mode.sh.
#
# Inputs (from parent dispatcher pre-tools/mod.sh, via `export`):
#   $tool_name - name of the tool being invoked
#   $input     - raw JSON payload on stdin

: "${tool_name:=}"
: "${input:=}"

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/detect.sh
. "$_toolu_lib/detect.sh"
# shellcheck source=../../lib/gate-mode.sh
. "$_toolu_lib/gate-mode.sh"

# MultiEdit is in the PreToolUse matcher and carries .tool_input.file_path just
# like Edit/Write — omitting it here would let a MultiEdit silently bypass the
# protected-files deny (a security-equivalent hole).
#
# Bash/Shell are also in scope: a structured Edit/Write on a protected path is
# denied, but the same bytes written via `sed -i`, a redirect, or `python3 -c
# "open(...).write(...)"` had no path-shaped tool_input to check and sailed
# through — the exact gap reported in
# github.com/Falconiere/toolu/issues/176. bash_write_targets (lib/detect.sh)
# extracts candidate write targets from the command; each is checked against
# the same protected-files.txt patterns as Edit/Write.
[[ "$tool_name" != "Edit" && "$tool_name" != "Write" && "$tool_name" != "MultiEdit" \
  && "$tool_name" != "Bash" && "$tool_name" != "Shell" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

TOOLU_SETTINGS_DIR=$(toolu_settings_dir)
LIST_FILE="$TOOLU_SETTINGS_DIR/protected-files.txt"

[ -f "$LIST_FILE" ] || exit 0

declare -a candidates=()
if [[ "$tool_name" == "Bash" || "$tool_name" == "Shell" ]]; then
  command_str=$(echo "$input" | jq -r '.tool_input.command // ""')
  [ -z "$command_str" ] && exit 0
  # Deduplicated: the same path can surface twice (once in the main command,
  # once via a command substitution the extractor recurses into), and checking
  # one path against the whole pattern list twice buys nothing.
  while IFS= read -r target; do
    [ -n "$target" ] || continue
    case " ${candidates[*]-} " in
      *" $target "*) continue ;;
    esac
    candidates+=("$target")
  done < <(bash_write_targets "$command_str")
  [ ${#candidates[@]} -eq 0 ] && exit 0
else
  file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""')
  [ -z "$file_path" ] && exit 0
  candidates=("$file_path")
fi

# read_list is sourced from lib/detect.sh.

# glob_match <pattern> <path>
glob_match() {
  local pattern="$1"
  local path="$2"
  # Enable extended globs for ** to match path segments.
  shopt -s extglob globstar 2>/dev/null || true
  # shellcheck disable=SC2053  # intentional glob match on RHS
  [[ "$path" == $pattern ]]
}

list=$(read_list "$LIST_FILE")

# Normalize absolute paths (Edit/Write sends absolute; a Bash write target is
# whatever the command wrote literally, often already relative) into
# repo-relative so patterns like "hooks/lib/**" can match. Falls back to the
# input unchanged outside a git repo (test sandboxes etc).
for candidate in "${candidates[@]}"; do
  rel_path=$(to_relative_path "$candidate")
  matched=""
  while IFS= read -r pattern; do
    [ -z "$pattern" ] && continue
    # Try as-is.
    if glob_match "$pattern" "$rel_path"; then
      matched="$pattern"
      break
    fi
    # For patterns that do not already start with **/, also try anchored
    # anywhere under the repo. This keeps "hooks/lib/**" matching even when
    # the path arrives as e.g. "subtree/hooks/lib/x.sh".
    if [[ "$pattern" != \*\*/* ]] && glob_match "**/$pattern" "$rel_path"; then
      matched="$pattern"
      break
    fi
    # Also check basename for patterns without a path separator.
    if [[ "$pattern" != */* ]]; then
      base=$(basename "$rel_path")
      if glob_match "$pattern" "$base"; then
        matched="$pattern"
        break
      fi
    fi
  done <<< "$list"

  if [ -n "$matched" ]; then
    mode=$(toolu_gate_mode protectedFiles)

    if [[ "$tool_name" == "Bash" || "$tool_name" == "Shell" ]]; then
      headline="This command would WRITE to $candidate, a protected path (matches \"$matched\")."
    else
      headline="Claude is trying to edit $candidate, a protected path (matches \"$matched\")."
    fi

    # Why THIS path is guarded, in the words that matter to the person
    # deciding. A generic "it is protected" teaches nothing and gets waved
    # through; naming the actual stake is the difference between a real
    # decision and a reflex.
    case "$rel_path" in
      *.env.example|*.env.template|*.env.sample)
        # Matched by the .env.* glob, but this one is meant to be committed.
        # Calling a template a secrets file is wrong, and a prompt that
        # overstates its case is one people learn to click through.
        detail="This is an example/template env file. It is committed on purpose, so it should carry placeholders and never live values — it is guarded because a real credential pasted here is a credential published to the repo." ;;
      .env|.env.*|*secrets*)
        detail="This is a secrets file. Approving lets an agent read or rewrite live credentials, and anything it writes here can leak into logs, commits, or a diff you push." ;;
      .git/*|*/.git/*)
        detail="This is git's internal state. Approving lets an agent rewrite refs, hooks, or config — including hooks that run on your machine at every commit." ;;
      *hooks/*|*skills/*)
        detail="This is toolu's own enforcement code — the hooks that run every other gate. Approving lets an agent edit the thing that is supposed to be watching it, which is how a guardrail gets quietly switched off." ;;
      *)
        detail="This path is listed in settings/protected-files.txt because edits to it are hard to notice and expensive to get wrong." ;;
    esac

    case "$mode" in
      ask)
        reason=$(toolu_gate_guardrail_warning "$headline" "$detail")
        ;;
      advise)
        reason="Protected path $candidate (matches \"$matched\"). $detail The write was NOT stopped — gates.protectedFiles.mode is 'advise'."
        ;;
      *)
        reason="$headline $detail Blocked by gates.protectedFiles.mode='block' (see plugins/toolu/hooks/docs/gates.md)."
        ;;
    esac

    toolu_gate_emit "$mode" "$reason"
    exit 0
  fi
done

exit 0
