#!/usr/bin/env bash
# Shared module dispatcher for the pre-tools / post-tools hook entrypoints.
# Source via:   . "${BASH_SOURCE%/*}/../lib/dispatch.sh"
#
# Public API:
#   toolu_dispatch_modules MODULES_DIR EVENT_NAME [REGISTRY_DIR...]
#     Runs every MODULES_DIR/*.sh in lexical order, then every *.sh in each
#     REGISTRY_DIR (built-in modules first), feeding each the exported `input`
#     variable (raw hook JSON) on stdin. EVENT_NAME is "PreToolUse" or
#     "PostToolUse" and selects the decision semantics.
#     Registry modules (anything outside MODULES_DIR) MUST be namespaced
#     "<plugin-spec>__<name>.sh" (specs must not contain "__"; "__" instead of
#     "." so specs like "name@git.example.com" parse unambiguously) and run
#     only when `toolu_plugin_active <plugin-spec>` succeeds. A registry
#     file that is not namespaced, or dispatched without that helper sourced,
#     is SKIPPED — fail closed, so a partial-sourcing bug degrades to "do
#     less" instead of running ungated modules. Built-in MODULES_DIR scripts
#     are never gated. The plugin-active lookup is memoized per spec within
#     one dispatch call.
#
# Output discipline:
#   - PreToolUse: a module emitting `hookSpecificOutput.permissionDecision ==
#     "deny"` is authoritative — its output is emitted immediately and
#     dispatch stops (security wins; a deny must not be suppressed by an
#     advisory).
#   - PostToolUse: a module emitting top-level `decision == "block"` is
#     authoritative — its output is emitted immediately and dispatch stops.
#     (PostToolUse has no permissionDecision; that is PreToolUse-only.)
#   - Otherwise every module's advisory `hookSpecificOutput.additionalContext`
#     (and top-level `systemMessage`) is collected and merged into ONE final
#     JSON object emitted at the end.
#
# Module exit-code semantics (deliberate — the old `echo "$input" | bash`
# pipeline masked these entirely):
#   - 0: stdout handled per the discipline above; stderr discarded.
#   - 2: the Claude Code "block via exit code" convention — the module's
#     stderr is forwarded and the dispatcher returns 2, so the entrypoint
#     exits 2 and Claude Code sees the block plus the stderr feedback.
#     Remaining modules are skipped.
#   - any other non-zero: treated as a module failure — one warning line on
#     stderr (visible in claude --debug), the module's possibly-partial
#     stdout is DISCARDED, and dispatch continues with the next module.

_TOOLU_DISPATCH_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=edit-records.sh
. "$_TOOLU_DISPATCH_LIB_DIR/edit-records.sh"

toolu_dispatch_modules() {
  # With <2 args `shift 2` would not shift at all, leaving $1 to be re-globbed
  # as a registry dir and every built-in module run twice.
  [[ $# -lt 2 ]] && return 0
  local modules_dir="$1" event="$2"; shift 2
  local registry_dirs=("$@")
  local script result rc err_file decision ctx msg c base plugin
  local contexts=() messages=()
  # Per-dispatch memo of plugin-active lookups (space-delimited spec lists;
  # plain strings, not associative arrays, for bash 3.2 compatibility).
  local active_specs=" " inactive_specs=" "

  # Constant for the whole dispatch; resolved once instead of per module.
  local plugin_helper_ok=0
  declare -F toolu_plugin_active >/dev/null 2>&1 && plugin_helper_ok=1

  err_file=$(mktemp "${TMPDIR:-/tmp}/toolu-dispatch.XXXXXX") || return 0

  # Ordered module list: built-in dir first, then each registry dir.
  local scripts=()
  for script in "$modules_dir"/*.sh; do
    [[ -f "$script" ]] && scripts+=("$script")
  done
  local rdir
  for rdir in "${registry_dirs[@]}"; do
    [[ -d "$rdir" ]] || continue
    for script in "$rdir"/*.sh; do
      [[ -f "$script" ]] && scripts+=("$script")
    done
  done

  for script in "${scripts[@]}"; do
    [[ ! -f "$script" ]] && continue

    # Anything outside MODULES_DIR is a registry module and MUST satisfy the
    # gating contract; violations are skipped, never run. Built-ins are
    # globbed directly from $modules_dir, so "registry" means the script's
    # dirname is not EXACTLY $modules_dir — a prefix match would let a
    # registry dir nested under the modules tree masquerade as built-in (and
    # an empty modules_dir prefix would match every absolute path).
    base=$(basename "$script")
    if [[ -z "$modules_dir" || "${script%/*}" != "$modules_dir" ]]; then
      # The spec must be non-empty ("__foo.sh") and whitespace-free: the
      # memo lists are space-delimited, so a spec containing whitespace
      # could substring-collide with another spec's memo entry.
      plugin="${base%%__*}"
      if [[ "$base" != *__*.sh || -z "$plugin" || "$plugin" == *[[:space:]]* ]]; then
        printf 'toolu-dispatch: registry module %s lacks <plugin-spec>__<name>.sh namespace; skipped\n' \
          "$base" >&2
        continue
      fi
      if [[ $plugin_helper_ok -ne 1 ]]; then
        printf 'toolu-dispatch: toolu_plugin_active not sourced; registry module %s skipped\n' \
          "$base" >&2
        continue
      fi
      # Resolve each distinct spec at most once per dispatch (hot path: one
      # jq per plugin instead of one per module).
      if [[ "$active_specs" != *" $plugin "* ]]; then
        [[ "$inactive_specs" == *" $plugin "* ]] && continue
        if ! toolu_plugin_active "$plugin"; then
          inactive_specs="${inactive_specs}${plugin} "
          continue
        fi
        active_specs="${active_specs}${plugin} "
      fi
    fi

    rc=0
    # Modules are always executed with `bash` regardless of their shebang. This
    # is fine today because only *.sh files are globbed above; a future non-bash
    # module would require this invocation to honor the script's interpreter.
    # shellcheck disable=SC2154 # $input is exported by the sourcing entrypoint.
    result=$(bash "$script" <<<"$input" 2>"$err_file") || rc=$?

    if [[ $rc -eq 2 ]]; then
      # Deliberate hard block: propagate stderr + exit code 2.
      cat "$err_file" >&2
      rm -f "$err_file"
      return 2
    fi
    if [[ $rc -ne 0 ]]; then
      # Module failure: make it visible, drop its partial output, keep going.
      printf 'toolu-dispatch: module %s exited %d; output skipped\n' \
        "$(basename "$script")" "$rc" >&2
      continue
    fi
    [[ -z "$result" ]] && continue

    case "$event" in
      PreToolUse)
        decision=$(jq -r '.hookSpecificOutput.permissionDecision // empty' <<<"$result" 2>/dev/null)
        if [[ "$decision" == "deny" ]]; then
          printf '%s\n' "$result"
          rm -f "$err_file"
          return 0
        fi
        ;;
      PostToolUse)
        decision=$(jq -r '.decision // empty' <<<"$result" 2>/dev/null)
        if [[ "$decision" == "block" ]]; then
          printf '%s\n' "$result"
          rm -f "$err_file"
          return 0
        fi
        ;;
    esac

    ctx=$(jq -r '.hookSpecificOutput.additionalContext // empty' <<<"$result" 2>/dev/null)
    if [[ -n "$ctx" ]]; then
      local seen=0 existing
      for existing in "${contexts[@]}"; do [[ "$existing" == "$ctx" ]] && seen=1; done
      [[ $seen -eq 0 ]] && contexts+=("$ctx")
    fi
    msg=$(jq -r '.systemMessage // empty' <<<"$result" 2>/dev/null)
    if [[ -n "$msg" ]]; then
      local seen_msg=0 existing_msg
      for existing_msg in "${messages[@]}"; do [[ "$existing_msg" == "$msg" ]] && seen_msg=1; done
      [[ $seen_msg -eq 0 ]] && messages+=("$msg")
    fi
  done
  rm -f "$err_file"

  local merged_ctx="" merged_msg=""
  if [[ ${#contexts[@]} -gt 0 ]]; then
    for c in "${contexts[@]}"; do
      if [[ -z "$merged_ctx" ]]; then
        merged_ctx="$c"
      else
        merged_ctx="${merged_ctx}"$'\n\n'"${c}"
      fi
    done
  fi
  if [[ ${#messages[@]} -gt 0 ]]; then
    for c in "${messages[@]}"; do
      if [[ -z "$merged_msg" ]]; then
        merged_msg="$c"
      else
        merged_msg="${merged_msg}"$'\n\n'"${c}"
      fi
    done
  fi

  if [[ -n "$merged_ctx" || -n "$merged_msg" ]]; then
    jq -n --arg ctx "$merged_ctx" --arg msg "$merged_msg" --arg ev "$event" '
      {}
      | (if $ctx != "" then .hookSpecificOutput = { hookEventName: $ev, additionalContext: $ctx } else . end)
      | (if $msg != "" then .systemMessage = $msg else . end)
    '
  fi

  return 0
}

# Host-aware dispatch wrapper. Non-edit tools preserve the single-dispatch
# path. Edit tools are normalized to one synthetic Edit payload per path; any
# per-path denial/block wins for the entire original patch, while exact
# duplicate advisories are emitted once.
toolu_dispatch_hook() {
  [[ $# -lt 2 ]] && return 0
  local modules_dir="$1" event="$2"; shift 2
  local registry_dirs=("$@")
  local original_input="${input:-}" original_tool="${tool_name:-}"
  local records normalize_rc=0 record path operation from moved_to synthetic
  local result rc decision ctx msg c existing seen
  local contexts=() messages=()

  records=$(toolu_normalize_edit_records "$original_input" "$original_tool") || normalize_rc=$?
  if [ "$normalize_rc" -eq 1 ] || [ "$normalize_rc" -eq 3 ]; then
    toolu_dispatch_modules "$modules_dir" "$event" "${registry_dirs[@]}"
    return $?
  fi
  if [ "$normalize_rc" -eq 2 ] || [ -z "$records" ]; then
    case "$event" in
      PreToolUse)
        printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Unable to parse apply_patch file headers; patch blocked so protected-file and quality gates cannot be bypassed."}}'
        ;;
      PostToolUse)
        printf '%s\n' '{"decision":"block","reason":"Unable to parse apply_patch file headers; per-file post-edit quality checks could not run."}'
        ;;
    esac
    return 0
  fi

  while IFS= read -r record; do
    [ -n "$record" ] || continue
    path=$(jq -er '.path | strings' <<<"$record" 2>/dev/null) || continue
    operation=$(jq -r '.operation // "update"' <<<"$record" 2>/dev/null)
    from=$(jq -r '.from // ""' <<<"$record" 2>/dev/null)
    moved_to=$(jq -r '.moved_to // ""' <<<"$record" 2>/dev/null)
    synthetic=$(jq -c --arg path "$path" --arg operation "$operation" \
      --arg from "$from" --arg moved_to "$moved_to" '
        .tool_name = "Edit"
        | .tool_input = ((.tool_input // {}) + {
            file_path: $path,
            path: $path,
            toolu_edit_operation: $operation,
            toolu_edit_from: $from,
            toolu_edit_moved_to: $moved_to
          })
      ' <<<"$original_input" 2>/dev/null) || continue

    input="$synthetic"
    tool_name=Edit
    TOOLU_EDIT_OPERATION="$operation"
    TOOLU_EDIT_FROM="$from"
    TOOLU_EDIT_MOVED_TO="$moved_to"
    export input tool_name TOOLU_EDIT_OPERATION TOOLU_EDIT_FROM TOOLU_EDIT_MOVED_TO

    rc=0
    result=$(toolu_dispatch_modules "$modules_dir" "$event" "${registry_dirs[@]}") || rc=$?
    if [ "$rc" -eq 2 ]; then
      input="$original_input"; tool_name="$original_tool"; export input tool_name
      unset TOOLU_EDIT_OPERATION TOOLU_EDIT_FROM TOOLU_EDIT_MOVED_TO
      return 2
    fi
    [ -n "$result" ] || continue

    case "$event" in
      PreToolUse)
        decision=$(jq -r '.hookSpecificOutput.permissionDecision // empty' <<<"$result" 2>/dev/null)
        ;;
      PostToolUse)
        decision=$(jq -r '.decision // empty' <<<"$result" 2>/dev/null)
        ;;
      *) decision="" ;;
    esac
    if [ "$decision" = deny ] || [ "$decision" = block ]; then
      input="$original_input"; tool_name="$original_tool"; export input tool_name
      unset TOOLU_EDIT_OPERATION TOOLU_EDIT_FROM TOOLU_EDIT_MOVED_TO
      printf '%s\n' "$result"
      return 0
    fi

    ctx=$(jq -r '.hookSpecificOutput.additionalContext // empty' <<<"$result" 2>/dev/null)
    if [ -n "$ctx" ]; then
      seen=0
      for existing in "${contexts[@]}"; do [ "$existing" = "$ctx" ] && seen=1; done
      [ "$seen" -eq 0 ] && contexts+=("$ctx")
    fi
    msg=$(jq -r '.systemMessage // empty' <<<"$result" 2>/dev/null)
    if [ -n "$msg" ]; then
      seen=0
      for existing in "${messages[@]}"; do [ "$existing" = "$msg" ] && seen=1; done
      [ "$seen" -eq 0 ] && messages+=("$msg")
    fi
  done <<<"$records"

  input="$original_input"; tool_name="$original_tool"; export input tool_name
  unset TOOLU_EDIT_OPERATION TOOLU_EDIT_FROM TOOLU_EDIT_MOVED_TO

  local merged_ctx="" merged_msg=""
  for c in "${contexts[@]}"; do
    if [ -z "$merged_ctx" ]; then merged_ctx="$c"; else merged_ctx="${merged_ctx}"$'\n\n'"${c}"; fi
  done
  for c in "${messages[@]}"; do
    if [ -z "$merged_msg" ]; then merged_msg="$c"; else merged_msg="${merged_msg}"$'\n\n'"${c}"; fi
  done
  if [ -n "$merged_ctx" ] || [ -n "$merged_msg" ]; then
    jq -n --arg ctx "$merged_ctx" --arg msg "$merged_msg" --arg ev "$event" '
      {}
      | (if $ctx != "" then .hookSpecificOutput = {hookEventName:$ev,additionalContext:$ctx} else . end)
      | (if $msg != "" then .systemMessage = $msg else . end)
    '
  fi
  return 0
}
