#!/usr/bin/env bash
# One-time host permission allowlist for a repo.
#
# toolu's gates are the layer that decides what should stop you; the host's
# permission prompts are a second, unrelated layer that stops you again for
# routine work — every git and gh invocation, every shell command. Once the
# gates ask properly (see gate-mode.sh), the second layer is mostly noise.
#
# So the first toolu session in a repo writes an allowlist into that repo's
# .claude/settings.local.json, and records a sentinel so it never does it
# again. Deleting a rule by hand therefore sticks: this is a one-time
# convenience, not a policy that reasserts itself every session.
#
# Deliberate choices:
#   * settings.local.json, not settings.json — the local file is not meant to
#     be committed, so a teammate never inherits this decision.
#   * the sentinel lives under the git-ignored state root, NOT in
#     toolu.config.json, which IS committed — a marker there would ride into
#     the repo and every clone would think it had already been done.
#   * entries are merged as a union. Nothing the user already wrote is
#     removed or reordered.
#   * a settings file we cannot parse is left byte-identical. Losing someone's
#     permission rules to a clumsy rewrite is far worse than a missing
#     convenience.
#
# Claude Code only: Codex has no equivalent permission file.
#
# Public API:
#   toolu_permissions_autowrite ROOT   0 if written, 1 if skipped

_TOOLU_PERMISSIONS_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=config.sh
. "$_TOOLU_PERMISSIONS_LIB_DIR/config.sh"

# Blanket shell access plus the two edit tools. Bash(*) subsumes Bash(git:*)
# and Bash(gh:*), so they are not listed separately.
TOOLU_PERMISSIONS_DEFAULT='["Bash(*)","Edit","Write"]'
TOOLU_PERMISSIONS_SENTINEL=".permissions-written"

_toolu_permissions_warn() {
  printf 'toolu-permissions: %s\n' "$1" >&2
}

# _toolu_permissions_entries -> the JSON array of rules to add.
# A configured permissions.allow replaces the default outright (a user who
# names their own set means that set, not that set plus ours).
_toolu_permissions_entries() {
  local configured
  toolu_load_config
  if [ "$_TOOLU_HAS_JQ" = "1" ]; then
    configured=$(jq -c '
      if ((.permissions?.allow? | type) == "array")
         and ((.permissions.allow | map(select(type == "string")) | length) > 0)
      then (.permissions.allow | map(select(type == "string")))
      else empty end' <<< "$TOOLU_CFG_JSON" 2>/dev/null)
    [ -n "$configured" ] && { printf '%s' "$configured"; return 0; }
  fi
  printf '%s' "$TOOLU_PERMISSIONS_DEFAULT"
}

# toolu_permissions_autowrite ROOT
toolu_permissions_autowrite() {
  local root="${1:-}" settings_file sentinel state_root entries existing merged tmp added
  [ -n "$root" ] || root=$(toolu_project_root)
  [ -n "$root" ] || return 1
  command -v jq >/dev/null 2>&1 || return 1

  # Claude-only.
  [ "$(toolu_host)" = claude ] || return 1

  # Explicit opt-out.
  toolu_flag_false permissions autoAllow && return 1

  # A directory that is not a repository has no project-local settings to own.
  git -C "$root" rev-parse --git-dir >/dev/null 2>&1 || return 1

  state_root=$(toolu_project_state_root "$root")
  [ -n "$state_root" ] || return 1
  sentinel="$state_root/$TOOLU_PERMISSIONS_SENTINEL"
  [ -f "$sentinel" ] && return 1

  settings_file="$root/$(toolu_project_dirname)/settings.local.json"
  existing='{}'
  if [ -f "$settings_file" ]; then
    if ! existing=$(jq -e . "$settings_file" 2>/dev/null); then
      _toolu_permissions_warn "malformed JSON in $settings_file; leaving it untouched"
      return 1
    fi
  fi

  entries=$(_toolu_permissions_entries)

  # Union, order-preserving: existing rules first, then whatever is new.
  merged=$(jq --argjson add "$entries" '
    ((.permissions.allow // []) | map(select(type == "string"))) as $have
    | ($add | map(select(. as $x | $have | index($x) | not))) as $new
    | .permissions = ((.permissions // {}) + {allow: ($have + $new)})
  ' <<< "$existing" 2>/dev/null) || {
    _toolu_permissions_warn "could not merge permissions into $settings_file"
    return 1
  }

  added=$(jq -r --argjson add "$entries" '
    ((.permissions.allow // []) | map(select(type == "string"))) as $have
    | ($add | map(select(. as $x | $have | index($x) | not)) | join(", "))
  ' <<< "$existing" 2>/dev/null)

  mkdir -p "$(dirname "$settings_file")" 2>/dev/null || return 1
  tmp=$(mktemp "${settings_file}.XXXXXX" 2>/dev/null) || return 1
  if ! printf '%s\n' "$merged" > "$tmp" 2>/dev/null || ! mv -f "$tmp" "$settings_file" 2>/dev/null; then
    rm -f "$tmp" 2>/dev/null
    _toolu_permissions_warn "could not write $settings_file"
    return 1
  fi

  # The sentinel is written only after the settings write actually landed, so a
  # failed write is retried next session rather than silently skipped forever.
  mkdir -p "$state_root" 2>/dev/null && : > "$sentinel" 2>/dev/null

  if [ -n "$added" ]; then
    printf 'toolu wrote %s to %s (one time only; delete a rule and it stays deleted). Add that file to .gitignore if it is not there already.\n' \
      "$added" "$settings_file"
  fi
  return 0
}
