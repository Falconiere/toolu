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
# shellcheck source=lib/registry.sh
. "$HOOK_DIR/lib/registry.sh"
# shellcheck source=lib/state-sweeper.sh
. "$HOOK_DIR/lib/state-sweeper.sh"
# shellcheck source=lib/permissions.sh
. "$HOOK_DIR/lib/permissions.sh"
# shellcheck source=lib/gate-mode.sh
. "$HOOK_DIR/lib/gate-mode.sh"

# Codex has no installed_plugins.json equivalent on the hot path. Snapshot the
# CLI's installed set once at SessionStart; all later checks consume this file.
toolu_snapshot_codex_plugins
toolu_registry_prune_inactive

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
_config_root="$(toolu_config_root)"
_old_sl="$_config_root/toolu/statusline.sh"
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
  # Model-tier placeholders: substituted from the resolved routing table so a
  # user who remaps a tier in toolu.config.json sees THEIR aliases in the
  # injected block, not the built-in defaults.
  content="${content//\{\{model_mechanical\}\}/$MODEL_MECHANICAL}"
  content="${content//\{\{model_exploration\}\}/$MODEL_EXPLORATION}"
  content="${content//\{\{model_implementation\}\}/$MODEL_IMPLEMENTATION}"
  content="${content//\{\{model_review\}\}/$MODEL_REVIEW}"
  content="${content//\{\{model_synthesis\}\}/$MODEL_SYNTHESIS}"
  content="${content//\{\{model_architecture\}\}/$MODEL_ARCHITECTURE}"
  content="${content//\{\{effort_mechanical\}\}/$EFFORT_MECHANICAL}"
  content="${content//\{\{effort_exploration\}\}/$EFFORT_EXPLORATION}"
  content="${content//\{\{effort_implementation\}\}/$EFFORT_IMPLEMENTATION}"
  content="${content//\{\{effort_review\}\}/$EFFORT_REVIEW}"
  content="${content//\{\{effort_synthesis\}\}/$EFFORT_SYNTHESIS}"
  content="${content//\{\{effort_architecture\}\}/$EFFORT_ARCHITECTURE}"
  printf '%s' "$content"
}

# Resolve the model-routing table once per session. Config-driven and stable
# within a session, so it is safe for the cached SessionStart prefix.
EFFORT_MECHANICAL="" EFFORT_EXPLORATION="" EFFORT_IMPLEMENTATION=""
EFFORT_REVIEW="" EFFORT_SYNTHESIS="" EFFORT_ARCHITECTURE=""
if [ "$(toolu_host)" = codex ]; then
  IFS=$'\t' read -r MODEL_MECHANICAL _effort <<<"$(toolu_codex_model mechanical)"
  EFFORT_MECHANICAL=", effort \`$_effort\`"
  IFS=$'\t' read -r MODEL_EXPLORATION _effort <<<"$(toolu_codex_model exploration)"
  EFFORT_EXPLORATION=", effort \`$_effort\`"
  IFS=$'\t' read -r MODEL_IMPLEMENTATION _effort <<<"$(toolu_codex_model implementation)"
  EFFORT_IMPLEMENTATION=", effort \`$_effort\`"
  IFS=$'\t' read -r MODEL_REVIEW _effort <<<"$(toolu_codex_model review)"
  EFFORT_REVIEW=", effort \`$_effort\`"
  IFS=$'\t' read -r MODEL_SYNTHESIS _effort <<<"$(toolu_codex_model synthesis)"
  EFFORT_SYNTHESIS=", effort \`$_effort\`"
  IFS=$'\t' read -r MODEL_ARCHITECTURE _effort <<<"$(toolu_codex_model architecture)"
  EFFORT_ARCHITECTURE=", effort \`$_effort\`"
else
  MODEL_MECHANICAL=$(toolu_model mechanical)
  MODEL_EXPLORATION=$(toolu_model exploration)
  MODEL_IMPLEMENTATION=$(toolu_model implementation)
  MODEL_REVIEW=$(toolu_model review)
  MODEL_SYNTHESIS=$(toolu_model synthesis)
  MODEL_ARCHITECTURE=$(toolu_model architecture)
fi

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

# Model routing — which tier handles which class of delegated work. Injected on
# every event (it must survive a compact, since post-compaction delegation is
# exactly where the agent otherwise falls back to one model for everything).
# Opt out with `"models": {"enabled": false}`.
if toolu_enabled models enabled; then
  routing_doc=$(render_doc "$HOOK_DIR/docs/model-routing.md")
  [ -n "$routing_doc" ] && parts+=("$routing_doc")
fi

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

# ── Housekeeping: reclaim spent state, and settle host permissions once ─────
# Both are best-effort and self-silencing; neither may keep a session from
# starting. The sweeper never speaks unless something went wrong; the
# permission writer speaks exactly once per repo, ever.
toolu_sweep_state "$PROJECT_ROOT"

_perm_summary=$(toolu_permissions_autowrite "$PROJECT_ROOT" 2>/dev/null || true)
[ -n "$_perm_summary" ] && parts+=("$_perm_summary")

# ── One-time notice: gates now ship relaxed ─────────────────────────────────
# Existing users had every gate blocking. The default is now the `balanced`
# preset, and a behavior change nobody was told about reads as a bug — so say
# it once per machine, to anyone who has not already made a choice about it.
_gate_notice_dir="$_config_root/toolu"
_gate_notice="$_gate_notice_dir/.gate-preset-notice-v5"
if [ ! -f "$_gate_notice" ] && [ "$_TOOLU_HAS_JQ" = "1" ] \
   && [ "$(jq -r 'has("gates")' <<< "$TOOLU_CFG_JSON" 2>/dev/null)" = "false" ]; then
  parts+=("toolu gates now default to the \`balanced\` preset: push-review ASKS instead of blocking (a yes is remembered for that diff), the quality gate blocks only \`git commit\`/\`git push\`, and commit-gate / docs-sync / plan-ledger advise. Pin the old behavior with \`{\"gates\":{\"preset\":\"strict\"}}\` in toolu.config.json, or tune one gate at a time with \`gates.<name>.mode\`.")
  mkdir -p "$_gate_notice_dir" 2>/dev/null && : > "$_gate_notice" 2>/dev/null
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
# When the comemory / ast-grep / exa-search / context7 plugins are INSTALLED
# and their underlying tool is available (binary on PATH, or published CLI
# wrapper), front-load a hard, proactive mandate into session context. The
# skills are ALWAYS-ACTIVE, but their bodies only load on trigger and the agent
# tends to wait to be asked — this injection makes the requirement unmissable
# from turn one. Aggressive by design; the per-skill opt-out (toolu_enabled
# skills <key> = false) is the escape hatch for a user who wants it off.
mandates=()
if [ "$HAS_COMEMORY" = "comemory" ] && toolu_enabled skills comemory && toolu_plugin_active comemory@toolu && toolu_flag_true comemory setup_done; then
  mandates+=("comemory (persistent memory) — you MUST, without being asked: (1) at the START of a task and BEFORE reading files, run \`comemory.sh search \"<topic>\"\` to recall prior decisions, bugs, patterns, and file-maps; (2) the MOMENT you make a decision, fix a bug, identify a pattern, or learn a reusable nuance, run \`comemory.sh save …\`; (3) when a \`search\` comes back EMPTY, do the native search (ast-grep/Grep), then SAVE what you find back so the next miss becomes a hit — the wrapper prints this reminder on every empty search. Treat recall+save as part of the task, never an optional extra, never something to ask permission for.")
elif [ "$HAS_COMEMORY" = "comemory" ] && toolu_enabled skills comemory && toolu_plugin_active comemory@toolu && [ "$_TOOLU_HAS_JQ" = "1" ] && ! toolu_flag_false comemory setup_done; then
  # comemory is installed + active but the user has not run /comemory:setup, so
  # the mandate stays OFF (opt-in). Nudge once toward setup instead. The jq
  # guard is load-bearing: without jq the setup_done marker can't be read, so
  # toolu_flag_true always reports false — gating the nudge on jq prevents a
  # perpetual "run setup" nudge on jq-less hosts where the flag is unreadable.
  # An explicit `comemory.setup_done: false` is a deliberate opt-out, not an
  # unanswered question — stay silent rather than re-nudging every session.
  parts+=("comemory detected but not enabled — run \`/comemory:setup\` to turn on persistent memory (recall/save) for this repo.")
fi
if [ "$HAS_ASTGREP" = "ast-grep" ] && toolu_enabled skills ast-grep && toolu_plugin_active ast-grep@toolu; then
  mandates+=("ast-grep (structural search) — for ANY search by code shape (signatures, call sites, impls, trait/interface usage, patterns) you MUST reach for \`ast-grep run --pattern …\` FIRST. Grep/ripgrep/sed are a FALLBACK ONLY — use them for plain-text literals in non-code files, or when a query genuinely cannot be expressed structurally. Never reach for them first on code.")
fi
# exa-search / context7 publish their CLI wrappers at a stable config-dir
# path via their own SessionStart hooks — there is no binary on PATH to
# detect, so the wrapper file itself is the presence signal. The symlink
# persists across sessions; only the very first session of a fresh install
# can race the publishing hook, and the mandate simply appears one session
# later. exa-search additionally gates on EXA_API_KEY: the wrapper hard-fails
# without it, and mandating a broken tool is worse than staying quiet.
exa_wrapper="$_config_root/exa-search/search.sh"
if [ -n "${EXA_API_KEY:-}" ] && [ -x "$exa_wrapper" ] && toolu_enabled skills exa-search && toolu_plugin_active exa-search@toolu; then
  mandates+=("exa-search (web search) — for ANY web search, code-example hunt, URL crawl, or topic research you MUST reach for \`\"$exa_wrapper\"\` FIRST (commands: search / crawl / similar — see the exa-search skill). Native web tools are a FALLBACK ONLY — use them when the wrapper errors or a URL needs your logged-in session.")
fi
ctx7_wrapper="$_config_root/context7/search.sh"
if [ -x "$ctx7_wrapper" ] && toolu_enabled skills context7 && toolu_plugin_active context7@toolu; then
  mandates+=("context7 (library docs) — for ANY third-party library/framework question (API usage, current docs, code examples, version behavior) you MUST query \`\"$ctx7_wrapper\"\` FIRST (\`search <library>\` to resolve the ID, then \`docs <id> <query>\`) BEFORE answering from memory or searching the web. Web search is a FALLBACK ONLY when context7 lacks coverage.")
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
plugin_root=$(toolu_plugin_root)
if [ "$(toolu_host)" = codex ] && [ -n "$plugin_root" ] && [ -f "$plugin_root/.codex-plugin/plugin.json" ]; then
  plugin_manifest="$plugin_root/.codex-plugin/plugin.json"
elif [ -n "$plugin_root" ] && [ -f "$plugin_root/.claude-plugin/plugin.json" ]; then
  plugin_manifest="$plugin_root/.claude-plugin/plugin.json"
elif [ -f "$PROJECT_ROOT/plugins/toolu/.claude-plugin/plugin.json" ]; then
  plugin_manifest="$PROJECT_ROOT/plugins/toolu/.claude-plugin/plugin.json"
fi
if [[ -n "$plugin_manifest" && -f "$plugin_manifest" ]] && command -v jq >/dev/null 2>&1; then
  dependency_manifest="$plugin_manifest"
  # Codex's published manifest schema has no dependency field. The Claude
  # manifest remains the shared dependency declaration inside each dual-host
  # plugin, so Codex reads it when its native manifest has no dependencies.
  if [ "$(toolu_host)" = codex ] && ! jq -e '.dependencies | type == "array"' \
      "$dependency_manifest" >/dev/null 2>&1 && \
      [ -f "$plugin_root/.claude-plugin/plugin.json" ]; then
    dependency_manifest="$plugin_root/.claude-plugin/plugin.json"
  fi
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
      missing_plugins+=("$(toolu_plugin_install_command "$req_spec")")
    fi
  done < <(jq -r '
    (.dependencies // [])[]
    | if type == "string" then .
      elif (type == "object" and (.name | type) == "string")
        then (if .marketplace then "\(.name)@\(.marketplace)" else .name end)
      else empty end
  ' "$dependency_manifest" 2>/dev/null)
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
