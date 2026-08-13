#!/usr/bin/env bash
# Normalize host-specific edit payloads into one JSON record per affected path.
# Records are newline-delimited JSON objects with at least `path` and
# `operation`. A move produces one source record and one destination record so
# protected-file and post-edit concerns inspect both sides.

toolu_is_edit_tool() {
  case "${1:-}" in
    Edit|Write|MultiEdit|apply_patch) return 0 ;;
    *) return 1 ;;
  esac
}

_toolu_patch_path_valid() {
  local path="${1:-}"
  [ -n "$path" ] || return 1
  case "$path" in
    *$'\n'*|*$'\r'*|*$'\t'*) return 1 ;;
  esac
  return 0
}

_toolu_edit_record() {
  local path="$1" operation="$2" relation_key="${3:-}" relation_value="${4:-}"
  if [ -n "$relation_key" ]; then
    jq -cn --arg path "$path" --arg operation "$operation" \
      --arg key "$relation_key" --arg value "$relation_value" \
      '{path:$path,operation:$operation} + {($key):$value}'
  else
    jq -cn --arg path "$path" --arg operation "$operation" \
      '{path:$path,operation:$operation}'
  fi
}

# Parse the apply_patch command carried by Codex. Return 2 with no output when
# framing or any file header is malformed; callers use that as a fail-closed
# PreToolUse signal.
_toolu_apply_patch_records() {
  local patch="$1" line raw path pending_update="" target
  local begun=0 ended=0 header_count=0 invalid=0
  local records=()

  while IFS= read -r line || [ -n "$line" ]; do
    line=${line%$'\r'}
    if [ "$ended" -eq 1 ]; then
      [ -z "$line" ] || invalid=1
      continue
    fi
    case "$line" in
      '*** Begin Patch')
        if [ "$begun" -ne 0 ]; then invalid=1; else begun=1; fi
        ;;
      '*** End Patch')
        if [ "$begun" -ne 1 ]; then
          invalid=1
        else
          if [ -n "$pending_update" ]; then
            records+=("$(_toolu_edit_record "$pending_update" update)")
            pending_update=""
          fi
          ended=1
        fi
        ;;
      '*** Add File:'*)
        [ "$begun" -eq 1 ] || { invalid=1; continue; }
        if [ -n "$pending_update" ]; then
          records+=("$(_toolu_edit_record "$pending_update" update)")
          pending_update=""
        fi
        raw=${line#'*** Add File:'}
        [[ "$raw" == ' '* ]] || { invalid=1; continue; }
        path=${raw# }
        _toolu_patch_path_valid "$path" || { invalid=1; continue; }
        records+=("$(_toolu_edit_record "$path" add)")
        header_count=$((header_count + 1))
        ;;
      '*** Update File:'*)
        [ "$begun" -eq 1 ] || { invalid=1; continue; }
        if [ -n "$pending_update" ]; then
          records+=("$(_toolu_edit_record "$pending_update" update)")
        fi
        raw=${line#'*** Update File:'}
        [[ "$raw" == ' '* ]] || { invalid=1; pending_update=""; continue; }
        path=${raw# }
        _toolu_patch_path_valid "$path" || { invalid=1; pending_update=""; continue; }
        pending_update="$path"
        header_count=$((header_count + 1))
        ;;
      '*** Delete File:'*)
        [ "$begun" -eq 1 ] || { invalid=1; continue; }
        if [ -n "$pending_update" ]; then
          records+=("$(_toolu_edit_record "$pending_update" update)")
          pending_update=""
        fi
        raw=${line#'*** Delete File:'}
        [[ "$raw" == ' '* ]] || { invalid=1; continue; }
        path=${raw# }
        _toolu_patch_path_valid "$path" || { invalid=1; continue; }
        records+=("$(_toolu_edit_record "$path" delete)")
        header_count=$((header_count + 1))
        ;;
      '*** Move to:'*)
        [ "$begun" -eq 1 ] && [ -n "$pending_update" ] || { invalid=1; continue; }
        raw=${line#'*** Move to:'}
        [[ "$raw" == ' '* ]] || { invalid=1; continue; }
        target=${raw# }
        _toolu_patch_path_valid "$target" || { invalid=1; continue; }
        records+=("$(_toolu_edit_record "$pending_update" update moved_to "$target")")
        records+=("$(_toolu_edit_record "$target" move from "$pending_update")")
        pending_update=""
        header_count=$((header_count + 1))
        ;;
      '*** End of File')
        # Optional Codex hunk marker: it describes EOF placement inside the
        # current file operation and does not introduce another affected path.
        [ "$begun" -eq 1 ] && [ "$header_count" -gt 0 ] || invalid=1
        ;;
      '*** '*)
        # Unknown apply_patch control header: never silently skip a path-shaped
        # operation that a newer patch grammar may introduce.
        invalid=1
        ;;
      *) ;;
    esac
  done <<<"$patch"

  if [ "$invalid" -ne 0 ] || [ "$begun" -ne 1 ] || [ "$ended" -ne 1 ] || [ "$header_count" -eq 0 ]; then
    return 2
  fi
  printf '%s\n' "${records[@]}"
}

# toolu_normalize_edit_records INPUT TOOL_NAME
#   0: emitted one or more records
#   1: TOOL_NAME is not an edit tool
#   2: edit payload is malformed
#   3: normalization dependency is unavailable
toolu_normalize_edit_records() {
  local raw_input="$1" edit_tool="$2" path operation patch
  toolu_is_edit_tool "$edit_tool" || return 1
  command -v jq >/dev/null 2>&1 || return 3

  if [ "$edit_tool" = apply_patch ]; then
    patch=$(jq -er '.tool_input.command | strings' <<<"$raw_input" 2>/dev/null) || return 2
    _toolu_apply_patch_records "$patch"
    return $?
  fi

  path=$(jq -er '.tool_input.file_path // .tool_input.path // .tool_input.target_file | strings' \
    <<<"$raw_input" 2>/dev/null) || return 2
  _toolu_patch_path_valid "$path" || return 2
  case "$edit_tool" in
    Write) operation="write" ;;
    *) operation="update" ;;
  esac
  _toolu_edit_record "$path" "$operation"
}
