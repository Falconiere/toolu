#!/usr/bin/env bash
# Resolve the docs-sync glob sets for the docs-sync pre-tools module. Mirrors
# quality-config.sh: a project/user override from the merged toolu config
# (.claude/toolu.config.json `docsSync.*`) wins, else a built-in default.
# Never errors — missing jq or config falls through to the defaults.
#
# Public API (each echoes a newline-separated glob list):
#   docs_sync_surfaces          - doc files whose change SATISFIES the check
#   docs_sync_surface_excludes  - doc paths that DO NOT satisfy it (carved back
#                                 out of `surfaces`; releases are per-release,
#                                 not per-task, and `*` crosses `/` so the
#                                 `docs/*.md` surface would otherwise swallow them)
#   docs_sync_code_surfaces     - code files whose change DEMANDS a doc touch
#
# Globs are matched by the module with bash `case` fnmatch, where a single `*`
# crosses `/` — so `docs/*.md` already covers nested paths and `*.ts` matches
# any depth. Author overrides with that semantics in mind.
#
# Source via:  . "$TOOLU_LIB_DIR/docs-sync-config.sh"

_DS_LIB_DIR="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}}"
# shellcheck source=./config.sh
[ -f "$_DS_LIB_DIR/config.sh" ] && . "$_DS_LIB_DIR/config.sh"

# Built-in defaults — single source of truth, overridable from the environment
# (tests set these to assert fall-through without touching call sites).
: "${DEFAULT_DOCS_SYNC_SURFACES:=README.md
*/README.md
docs/*.md
*/SKILL.md
AGENTS.md
*/AGENTS.md
CLAUDE.md
*/CLAUDE.md
*/workflows/*.md}"
: "${DEFAULT_DOCS_SYNC_SURFACE_EXCLUDES:=docs/releases/*
*/docs/releases/*}"
: "${DEFAULT_DOCS_SYNC_CODE_SURFACES:=*.ts
*.rs
*.sh
*/commands/*
*plugin.json
*.config.json}"

# _ds_override KEY -> newline-joined override globs, or "" when unset/empty.
# Reads docsSync.<key> as a JSON array of strings from the merged config.
_ds_override() {
  local key="$1"
  command -v toolu_load_config >/dev/null 2>&1 || return 0
  toolu_load_config
  [ "${_TOOLU_HAS_JQ:-0}" = "1" ] || return 0
  [ -n "${TOOLU_CFG_JSON:-}" ] || return 0
  jq -r --arg k "$key" '
    (.docsSync? // {})[$k]?
    | if type=="array" then .[] else empty end
  ' <<< "$TOOLU_CFG_JSON" 2>/dev/null
}

# docs_sync_surfaces -> doc-surface globs (override -> default).
docs_sync_surfaces() {
  local v
  v=$(_ds_override surfaces)
  [ -n "$v" ] && { printf '%s\n' "$v"; return 0; }
  printf '%s\n' "$DEFAULT_DOCS_SYNC_SURFACES"
}

# docs_sync_surface_excludes -> doc paths carved back out (override -> default).
docs_sync_surface_excludes() {
  local v
  v=$(_ds_override surfaceExcludes)
  [ -n "$v" ] && { printf '%s\n' "$v"; return 0; }
  printf '%s\n' "$DEFAULT_DOCS_SYNC_SURFACE_EXCLUDES"
}

# docs_sync_code_surfaces -> code-surface globs (override -> default).
docs_sync_code_surfaces() {
  local v
  v=$(_ds_override codeSurfaces)
  [ -n "$v" ] && { printf '%s\n' "$v"; return 0; }
  printf '%s\n' "$DEFAULT_DOCS_SYNC_CODE_SURFACES"
}
