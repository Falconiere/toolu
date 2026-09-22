#!/usr/bin/env bash
# Install toolu's Codex custom-agent profiles without clobbering user files.
set -euo pipefail

SELF_DIR=$(cd "${BASH_SOURCE[0]%/*}" && pwd)
PLUGIN_ROOT=$(cd "$SELF_DIR/../../.." && pwd)
TEMPLATE_DIR="${TOOLU_AGENT_TEMPLATE_DIR:-$PLUGIN_ROOT/assets/agents}"
CODEX_ROOT="${CODEX_HOME:-${HOME:+$HOME/.codex}}"
AGENTS_DIR="${CODEX_ROOT:+$CODEX_ROOT/agents}"
MANAGED_MARKER='# Managed by toolu. Install and update with $toolu:setup.'
NAMES=(quick-task deep-explore research-agent implementer architect)
MODELS=(gpt-5.6-luna gpt-5.6-terra gpt-5.6-terra gpt-5.6-terra gpt-5.6-sol)
EFFORTS=(medium medium medium medium high)
SANDBOXES=(read-only read-only read-only workspace-write read-only)

usage() {
  printf '%s\n' 'Usage: setup.sh preview | install [--force] | remove --yes [--force]'
}

fail() {
  printf 'toolu setup: %s\n' "$*" >&2
  exit 1
}

validate_template() {
  local index="$1" name file
  name="${NAMES[$index]}"
  file="$TEMPLATE_DIR/$name.toml"
  [ -f "$file" ] || return 1
  [ "$(grep -Fxc "$MANAGED_MARKER" "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Fxc "name = \"$name\"" "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Ec '^description = "[^"].*"$' "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Fxc "model = \"${MODELS[$index]}\"" "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Fxc "model_reasoning_effort = \"${EFFORTS[$index]}\"" "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Fxc "sandbox_mode = \"${SANDBOXES[$index]}\"" "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Fxc 'developer_instructions = """' "$file" || true)" -eq 1 ] || return 1
  [ "$(grep -Fxc '"""' "$file" || true)" -eq 1 ] || return 1
  awk '
    BEGIN { block=0; assignments=0; starts=0; closes=0; invalid=0 }
    /^#[[:space:]]*/ || /^[[:space:]]*$/ { next }
    block {
      if ($0 == "\"\"\"") { block=0; closes++; next }
      next
    }
    $0 == "developer_instructions = \"\"\"" { block=1; starts++; next }
    /^(name|description|model|model_reasoning_effort|sandbox_mode) = "[^"\\]*(\\.[^"\\]*)*"$/ {
      assignments++; next
    }
    { invalid=1 }
    END { exit(invalid || block || assignments != 5 || starts != 1 || closes != 1) }
  ' "$file" || return 1
}

classify_install() {
  local name="$1" source="$TEMPLATE_DIR/$1.toml" target="$AGENTS_DIR/$1.toml"
  if [ ! -e "$target" ]; then
    printf 'install'
  elif cmp -s "$source" "$target"; then
    printf 'unchanged'
  elif grep -Fqx "$MANAGED_MARKER" "$target" 2>/dev/null; then
    printf 'update'
  else
    printf 'conflict'
  fi
}

classify_remove() {
  local target="$AGENTS_DIR/$1.toml"
  if [ ! -e "$target" ]; then
    printf 'absent'
  elif grep -Fqx "$MANAGED_MARKER" "$target" 2>/dev/null; then
    printf 'remove'
  else
    printf 'conflict'
  fi
}

backup_stamp() {
  local stamp="${TOOLU_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
  case "$stamp" in
    *[!0-9TZ]*|'') fail "invalid backup timestamp: $stamp" ;;
  esac
  printf '%s' "$stamp"
}

command_name="${1:-}"
shift 2>/dev/null || true
force=0
confirmed=0
for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    --yes) confirmed=1 ;;
    *) usage >&2; exit 2 ;;
  esac
done

case "$command_name" in
  preview|install|remove) ;;
  *) usage >&2; exit 2 ;;
esac
[ -n "$AGENTS_DIR" ] || fail 'CODEX_HOME and HOME are both unset'

for index in 0 1 2 3 4; do
  validate_template "$index" || fail "invalid agent template: $TEMPLATE_DIR/${NAMES[$index]}.toml"
done

printf 'TARGET %s\n' "$AGENTS_DIR"
actions=()
conflicts=0
for name in "${NAMES[@]}"; do
  if [ "$command_name" = remove ]; then
    action=$(classify_remove "$name")
  else
    action=$(classify_install "$name")
  fi
  actions+=("$action")
  printf 'PLAN %s %s\n' "$name" "$action"
  [ "$action" != conflict ] || conflicts=$((conflicts + 1))
done

if [ "$command_name" = preview ]; then
  [ "$conflicts" -eq 0 ] || printf 'NOTICE use install --force only after confirming each conflict\n'
  exit 0
fi

if [ "$command_name" = remove ] && [ "$confirmed" -ne 1 ]; then
  printf '%s\n' 'REFUSED removal requires --yes after explicit user confirmation' >&2
  exit 2
fi
if [ "$conflicts" -gt 0 ] && [ "$force" -ne 1 ]; then
  printf '%s\n' 'REFUSED unmanaged profile conflict; preview it and confirm install/remove --force' >&2
  exit 2
fi

stamp=""
backup_dir=""
needs_backup=0
for action in "${actions[@]}"; do
  case "$action" in update|remove|conflict) needs_backup=1 ;; esac
done
if [ "$needs_backup" -eq 1 ]; then
  stamp=$(backup_stamp)
  backup_dir="$AGENTS_DIR/.toolu-backups/$stamp"
  for index in 0 1 2 3 4; do
    action="${actions[$index]}"
    case "$action" in
      update|remove|conflict)
        [ ! -e "$backup_dir/${NAMES[$index]}.toml" ] || fail "backup already exists: $backup_dir/${NAMES[$index]}.toml"
        ;;
    esac
  done
fi

mkdir -p "$AGENTS_DIR"
[ "$needs_backup" -eq 0 ] || mkdir -p "$backup_dir"

if [ "$command_name" = install ]; then
  installed=0
  updated=0
  unchanged=0
  for index in 0 1 2 3 4; do
    name="${NAMES[$index]}"
    action="${actions[$index]}"
    source="$TEMPLATE_DIR/$name.toml"
    target="$AGENTS_DIR/$name.toml"
    case "$action" in
      unchanged)
        unchanged=$((unchanged + 1))
        ;;
      update|conflict)
        cp -p "$target" "$backup_dir/$name.toml"
        tmp="$AGENTS_DIR/.$name.toml.toolu.$$"
        cp "$source" "$tmp"
        mv "$tmp" "$target"
        updated=$((updated + 1))
        ;;
      install)
        tmp="$AGENTS_DIR/.$name.toml.toolu.$$"
        cp "$source" "$tmp"
        mv "$tmp" "$target"
        installed=$((installed + 1))
        ;;
    esac
  done
  printf 'INSTALLED %d UPDATED %d UNCHANGED %d\n' "$installed" "$updated" "$unchanged"
else
  removed=0
  absent=0
  for index in 0 1 2 3 4; do
    name="${NAMES[$index]}"
    action="${actions[$index]}"
    target="$AGENTS_DIR/$name.toml"
    case "$action" in
      absent) absent=$((absent + 1)) ;;
      remove|conflict)
        mv "$target" "$backup_dir/$name.toml"
        removed=$((removed + 1))
        ;;
    esac
  done
  printf 'REMOVED %d ABSENT %d\n' "$removed" "$absent"
fi

[ -z "$backup_dir" ] || printf 'BACKUP %s\n' "$backup_dir"
printf '%s\n' 'Restart Codex to reload custom agent profiles.'
