#!/usr/bin/env bash
# SessionStart hook
# Event-aware: tailors context for startup, resume, clear, compact.
# Project-agnostic: detects project name, language, and package manager.

HOOK_DIR="$(cd "$(dirname "$0")" && pwd)"

# Disable bash 5.2+ patsub_replacement so `&` in ${var//pat/repl} values is
# literal. No-op (option unknown) on older bash, including macOS /bin/bash 3.2.
shopt -u patsub_replacement 2>/dev/null || true

# shellcheck source=lib/detect.sh
. "$HOOK_DIR/lib/detect.sh"
# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

# Transitional orphan sweep (safe to delete this block once users have migrated,
# ~v1.7+): the statusline moved to the standalone `statusline` plugin
# (~/.claude/statusline/statusline.sh, wired by its own SessionStart hook). Runs
# every session but no-ops once the symlink is gone. Remove the stale symlink
# toolu used to own at
# $config/toolu/statusline.sh so an un-migrated settings.json fails loudly
# (missing file) instead of dangling into a cleaned plugin cache. Only ever
# removes OUR symlink — a real file a user placed there is left untouched.
# Deliberately runs BEFORE the `toolu_enabled` opt-out below: a user who
# disables session-start context should still not be left with a dangling symlink.
_old_sl="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/toolu/statusline.sh"
[ -L "$_old_sl" ] && rm -f "$_old_sl"

if ! toolu_enabled hooks session-start; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

PROJECT_ROOT="$(detect_project_root)"
[ -z "$PROJECT_ROOT" ] && PROJECT_ROOT="$(pwd)"
PROJECT_NAME="$(detect_project_name)"
NODE_PM="$(detect_node_pm)"
HAS_RUST="$(detect_rust)"
HAS_TS="$(detect_ts)"

# ── Parse stdin to detect event type ────────────────────────────────────────
input=$(cat 2>/dev/null || echo "{}")
event="startup"
if command -v jq >/dev/null 2>&1; then
  # Claude Code sends the SessionStart event type in `source`
  # (startup | resume | clear | compact); keep legacy fields as fallbacks.
  event=$(jq -r '.source // .session_event // .event // "startup"' <<< "$input" 2>/dev/null || echo "startup")
fi
[[ -z "$event" || "$event" == "null" ]] && event="startup"

# ── Render the main session doc with project tokens substituted ─────────────
render_doc() {
  local src="$1"
  [ -f "$src" ] || { echo ""; return 0; }
  local content
  content=$(cat "$src")
  # Substitute placeholders with bash-native replacement — immune to sed
  # metacharacters (|, &, \) in project names or package manager values.
  # Replacement is deliberately UNQUOTED: quoting inside ${} inserts literal
  # quote characters on bash 3.2 (stock macOS). Literal-& safety on bash 5.2+
  # comes from `shopt -u patsub_replacement` at the top of this script.
  local name="${PROJECT_NAME:-this project}"
  local pm="${NODE_PM:-your package manager}"
  content="${content//\{\{project_name\}\}/$name}"
  content="${content//\{\{node_pm\}\}/$pm}"
  printf '%s' "$content"
}

# ── Git context (branch only) ───────────────────────────────────────────────
# Cache discipline: this string lands in the once-cached SessionStart prefix, so
# it must depend only on stable inputs. The branch is stable within a session;
# the working-tree dirty count is NOT — it goes stale immediately and differs on
# every startup/resume/compact, perturbing the prefix for no lasting value. Live
# working state is already on the statusline and one `git status` away.
git_ctx=""
if command -v git >/dev/null 2>&1; then
  branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  [[ -n "$branch" ]] && git_ctx="Branch: $branch"
fi

# Note: the failing-quality-gate reminder is deliberately NOT injected here. It
# is volatile (flips as you work) and already surfaced live by the statusline
# and per-prompt by user-prompt-submit.sh — re-injecting it into the cached
# prefix would duplicate context and perturb the cache on every gate flip.

# ── Build context based on event type ───────────────────────────────────────
parts=()
title=""

main_doc=$(render_doc "$HOOK_DIR/docs/session-start.md")

case "$event" in
  startup)
    title="Toolu is on!"
    [ -n "$main_doc" ] && parts+=("$main_doc")
    ;;
  resume)
    title="Session resumed"
    [ -n "$main_doc" ] && parts+=("$main_doc")
    ;;
  clear)
    title="Context cleared"
    [ -n "$main_doc" ] && parts+=("$main_doc")
    ;;
  compact)
    title="Context compacted"
    compaction=$(cat "$HOOK_DIR/docs/post-compaction.md" 2>/dev/null || echo "")
    [[ -n "$compaction" ]] && parts+=("$compaction")
    [ -n "$main_doc" ] && parts+=("$main_doc")
    ;;
esac

# Per-toolchain snippets — opt-in via $TOOLU_VERBOSE to save tokens.
# Default off: the session-start.md core already covers project rules.
# Set TOOLU_VERBOSE=1 to re-enable per-toolchain blocks. Any value other
# than "0" / unset / empty enables (so `=true`, `=on`, `=1` all work, but a
# user who sets `=0` to disable is not surprised).
if [ "${TOOLU_VERBOSE:-0}" != "0" ]; then
  if [ "$HAS_TS" = "ts" ]; then
    ts_doc=$(render_doc "$HOOK_DIR/docs/session-start-ts.md")
    [ -n "$ts_doc" ] && parts+=("$ts_doc")
  fi
  if [ "$HAS_RUST" = "rust" ]; then
    rust_doc=$(render_doc "$HOOK_DIR/docs/session-start-rust.md")
    [ -n "$rust_doc" ] && parts+=("$rust_doc")
  fi
fi

# Append project line only when name was detected.
if [ -n "$PROJECT_NAME" ]; then
  parts+=("Project: $PROJECT_NAME")
fi

# Warn when optional tools referenced by docs/skills are missing — keeps the
# session start honest about which capabilities are actually available.
HAS_COMEMORY="$(detect_comemory)"
HAS_ASTGREP="$(detect_ast_grep)"
missing_tools=()
if [ "$HAS_COMEMORY" != "comemory" ] && toolu_enabled skills comemory; then
  missing_tools+=("comemory (persistent memory recall/save)")
fi
# Present but outdated: toolu relies on comemory's full verb surface — an
# older binary lacks the retrieval-loop / code-search verbs and will error on
# them. Advisory only (non-fatal); the basics still work.
if [ "$HAS_COMEMORY" = "comemory" ] && toolu_enabled skills comemory && ! comemory_version_ok; then
  parts+=("WARN: comemory $(comemory_version) is older than the v$COMEMORY_MIN_VERSION toolu targets — feedback/mine/tune/search-code/graph may be unavailable. Upgrade: \`brew upgrade Falconiere/tap/comemory\` (comemory is not on crates.io).")
fi
if [ "$HAS_ASTGREP" != "ast-grep" ] && toolu_enabled skills ast-grep; then
  missing_tools+=("ast-grep (structural code search)")
fi
if [ "${#missing_tools[@]}" -gt 0 ]; then
  warn="WARN: optional tools missing — features that depend on them are disabled:"
  for t in "${missing_tools[@]}"; do
    warn+="
  • $t"
  done
  parts+=("$warn")
fi

# ── Mandatory proactive tool use ────────────────────────────────────────────
# When the comemory / ast-grep plugins are INSTALLED and their underlying tool
# is on PATH, front-load a hard, proactive mandate into session context. The
# skills are ALWAYS-ACTIVE, but their bodies only load on trigger and the agent
# tends to wait to be asked — this injection makes the requirement unmissable
# from turn one. Aggressive by design; the per-skill opt-out (toolu_enabled
# skills <key> = false) is the escape hatch for a user who wants it off.
mandates=()
if [ "$HAS_COMEMORY" = "comemory" ] && toolu_enabled skills comemory && toolu_plugin_active comemory@toolu; then
  mandates+=("comemory (persistent memory) — you MUST, without being asked: (1) at the START of a task and BEFORE reading files, run \`comemory.sh search \"<topic>\"\` to recall prior decisions, bugs, patterns, and file-maps; (2) the MOMENT you make a decision, fix a bug, identify a pattern, or learn a reusable nuance, run \`comemory.sh save …\`; (3) when a \`search\` comes back EMPTY, do the native search (ast-grep/Grep), then SAVE what you find back so the next miss becomes a hit — the wrapper prints this reminder on every empty search. Treat recall+save as part of the task, never an optional extra, never something to ask permission for.")
fi
if [ "$HAS_ASTGREP" = "ast-grep" ] && toolu_enabled skills ast-grep && toolu_plugin_active ast-grep@toolu; then
  mandates+=("ast-grep (structural search) — for ANY search by code shape (signatures, call sites, impls, trait/interface usage, patterns) you MUST reach for \`ast-grep run --pattern …\` FIRST. Grep/ripgrep/sed are a FALLBACK ONLY — use them for plain-text literals in non-code files, or when a query genuinely cannot be expressed structurally. Never reach for them first on code.")
fi
if [ "${#mandates[@]}" -gt 0 ]; then
  mwarn="MANDATORY — proactive plugin use (installed + available; NOT optional, do NOT ask permission):"
  for m in "${mandates[@]}"; do
    mwarn+="
  • $m"
  done
  mwarn+="
  • Propagation: these mandates bind EVERY agent in this session, not just the main thread. When you delegate via Task/subagents (nested ones too), carry the same requirement into the subagent's prompt — recall/save via comemory, ast-grep first with Grep/sed as fallback. Delegation never exempts the work."
  parts+=("$mwarn")
fi

# Verify required plugin dependencies declared in plugin.json `dependencies`
# (Claude Code's official schema: array of "name" strings or
# {name, marketplace?, version?} objects). Auto-install only fires when the
# dep's marketplace is already configured; we surface the exact
# `/plugin install …` command so the user can fix in one paste.
#
# Scope: the check only fires when we can locate the toolu plugin manifest
# (via CLAUDE_PLUGIN_ROOT, set by Claude Code when this hook runs from the
# installed plugin, or via the in-repo path when working inside the toolu
# checkout itself). Outside both — e.g., a hook run by another project that
# inherits these helpers — the block silently no-ops. Intentional: repo-A
# should not WARN about plugin deps declared in repo-B's manifest.
plugin_manifest=""
if [ -n "${CLAUDE_PLUGIN_ROOT:-}" ] && [ -f "$CLAUDE_PLUGIN_ROOT/.claude-plugin/plugin.json" ]; then
  plugin_manifest="$CLAUDE_PLUGIN_ROOT/.claude-plugin/plugin.json"
elif [ -f "$PROJECT_ROOT/plugins/toolu/.claude-plugin/plugin.json" ]; then
  plugin_manifest="$PROJECT_ROOT/plugins/toolu/.claude-plugin/plugin.json"
fi
if [[ -n "$plugin_manifest" && -f "$plugin_manifest" ]] && command -v jq >/dev/null 2>&1; then
  missing_plugins=()
  indeterminate=0
  while IFS= read -r req_spec; do
    [ -z "$req_spec" ] && continue
    installed=$(detect_plugin_installed "$req_spec")
    rc=$?
    if [ "$rc" -eq 2 ]; then
      # Registry/jq unavailable on this box — suppress all WARNs in this
      # block so we don't spam every required plugin as "missing" on a
      # machine where the registry was moved or jq was uninstalled.
      indeterminate=1
      break
    fi
    if [ -z "$installed" ]; then
      missing_plugins+=("/plugin install ${req_spec}")
    fi
  done < <(jq -r '
    (.dependencies // [])[]
    | if type == "string" then .
      elif (type == "object" and (.name | type) == "string")
        then (if .marketplace then "\(.name)@\(.marketplace)" else .name end)
      else empty end
  ' "$plugin_manifest" 2>/dev/null)
  if [ "$indeterminate" -eq 0 ] && [ "${#missing_plugins[@]}" -gt 0 ]; then
    pwarn="WARN: required plugins missing — review/simplify pipelines will fail. Install:"
    for cmd in "${missing_plugins[@]}"; do
      pwarn+="
  • $cmd"
    done
    parts+=("$pwarn")
  fi
fi

# Append git context (branch only — see the cache-discipline note above).
[[ -n "$git_ctx" ]] && parts+=("$git_ctx")

# ── Join and output ─────────────────────────────────────────────────────────
full_context=""
for part in "${parts[@]}"; do
  if [[ -z "$full_context" ]]; then
    full_context="$part"
  else
    full_context="$full_context

$part"
  fi
done

if command -v jq >/dev/null 2>&1; then
  jq -n --arg ctx "$full_context" --arg title "$title" '{
    "hookSpecificOutput": {
      "hookEventName": "SessionStart",
      "additionalContext": $ctx
    },
    "systemMessage": $title
  }'
fi

exit 0
