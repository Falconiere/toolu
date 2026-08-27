#!/usr/bin/env bash
# UserPromptSubmit hook
# Validates prompts, injects optional per-project context, git context, intent hints.
# Project-agnostic: no project literals. Per-project hints opt in via the
# host-native project directory's context.sh.

HOOK_DIR="$(cd "$(dirname "$0")" && pwd)"

# shellcheck source=lib/detect.sh
. "$HOOK_DIR/lib/detect.sh"
# shellcheck source=lib/config.sh
. "$HOOK_DIR/lib/config.sh"

if ! toolu_enabled hooks user-prompt-submit; then
  cat > /dev/null 2>&1 || true
  exit 0
fi

input=$(cat)
prompt=""
if command -v jq >/dev/null 2>&1; then
  prompt=$(jq -r '.prompt // ""' <<< "$input" 2>/dev/null || echo "")
fi

[[ -z "$prompt" ]] && exit 0

PROJECT_ROOT="$(detect_project_root)"
[ -z "$PROJECT_ROOT" ] && PROJECT_ROOT="$(pwd)"
GATE_FILE="$(toolu_project_state_root "$PROJECT_ROOT")/quality-gate-status.json"
prompt_lower=$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')

# ── Skip trivial prompts (confirmations, short replies) ───────────────────────
if [[ "$prompt_lower" =~ ^(y|n|yes|no|ok|sure|thanks|thank\ you|go\ ahead|looks\ good|lgtm|correct|exactly|right|done|nah|nope|yep|yup|continue)[\.\!\?]?$ ]]; then
  exit 0
fi

# ── Skip slash commands — skills handle their own context ─────────────────────
if [[ "$prompt_lower" =~ ^/ ]]; then
  exit 0
fi

# ── Block vague one-word prompts ──────────────────────────────────────────────
if [[ "$prompt_lower" =~ ^(fix|help|debug|check|look|see|run|do|try)[[:space:]]*$ ]]; then
  if command -v jq >/dev/null 2>&1; then
    jq -n '{"decision": "block", "reason": "Prompt too vague - specify what file/feature/error needs attention"}'
  fi
  exit 0
fi

# Word-boundary helpers — bash extended regex has no `\b`. WB/WE wrap each
# alternation so e.g. `impl` does NOT match `implement` and `move` does NOT
# match `remove`. Also dropped overly short tokens that no boundary trick
# can rescue: `impl` (vs implement), `ast` (vs fast/past/last), `drop`
# (vs dropdown). Inflected forms (tested, fixing) won't match — users
# typically write the base verb in a directive prompt, and silence is
# safer than the wrong hint.
WB='(^|[^a-z])'
WE='([^a-z]|$)'

# ── Quality gate warning (not block) ─────────────────────────────────────────
quality_gate_hint=""
if [[ -f "$GATE_FILE" ]] && command -v jq >/dev/null 2>&1; then
  gate_json=$(cat "$GATE_FILE" 2>/dev/null)
  gate_status=$(jq -r '.status // ""' <<< "$gate_json" 2>/dev/null)
  if [[ "$gate_status" == "failing" ]]; then
    # Word-boundary wrapped: `prefix` must not suppress via the `fix` substring.
    if ! [[ "$prompt_lower" =~ ${WB}(fix|resolve|error|warning|test|lint|check|type)${WE} ]]; then
      reason=$(jq -r '.reason // "Unknown quality failure"' <<< "$gate_json" 2>/dev/null)
      quality_gate_hint="Quality gate failing: $reason. Prefer fixing before unrelated work."
    fi
  fi
fi

# ── Build context parts ──────────────────────────────────────────────────────
# Token-budget rule: every part below must be opt-in via prompt content.
# We do NOT re-inject SessionStart material (branch, gates, recall protocol)
# on every prompt — that duplicates context already in the session.

HAS_ASTGREP="$(detect_ast_grep)"
if ! toolu_enabled skills ast-grep; then
  HAS_ASTGREP=""
fi

# 1. Memory recall hint — on explicit recall words AND ordinary task verbs.
# Task verbs (add/implement/build/…) are included deliberately: starting work on
# a feature is exactly when prior decisions/file-maps should be recalled first.
# WB/WE word-boundary wrapping keeps base verbs from matching as substrings
# (`add` not `address`, `build` not `rebuild`, `create` not `created`). `fix`,
# `debug`, `bug` are intentionally NOT here — they already drive the intent hint
# below; recall + intent may both fire, which is fine (they say different things).
# (WB/WE word-boundary helpers are defined above the quality-gate block.)
recall=""
if [[ "$prompt_lower" =~ ${WB}(remember|recall|what\ did|previously|earlier|comemory|architecture|how\ does|where\ is|file-map|prior\ decision|history|add|implement|build|create|write|update|change|refactor|migrate|rename)${WE} ]]; then
  case "$(toolu_comemory_state)" in
    available)
      # Emit the STABLE published path register.sh symlinks into, not a bare
      # `comemory.sh` — the wrapper is not on PATH by design, so the bare form
      # dies with command-not-found and the agent reads that as "no memories".
      recall="Recall first: \`\"$(toolu_config_root)/comemory/comemory.sh\" search \"<topic>\"\` before reading files."
      ;;
    missing)
      recall="WARN: comemory CLI not installed — persistent memory recall disabled."
      ;;
  esac
fi

# 2. Intent hint — at most ONE. Most-specific pattern wins.
intent=""
if [[ "$prompt_lower" =~ ${WB}(pattern|struct|trait|interface|all\ functions|all\ methods|every\ function|every\ method|syntax|code\ structure|signature|return\ type|where\ clause|lifetime|closure|macro|decorator|annotation)${WE} ]]; then
  if [ "$HAS_ASTGREP" = "ast-grep" ]; then
    intent="Structural pattern: use \`ast-grep run --pattern\` (not Grep)."
  else
    intent="WARN: ast-grep not installed — install via brew/cargo for structural matching."
  fi
elif [[ "$prompt_lower" =~ ${WB}(rename|move|extract|split)${WE} ]]; then
  intent="Rename: find all refs (ast-grep + Grep on configs) before rewriting."
elif [[ "$prompt_lower" =~ ${WB}(test|spec|coverage)${WE} ]]; then
  intent="Tests: real-world data only, NO mocks."
elif [[ "$prompt_lower" =~ ${WB}(fix|debug|error|bug|issue)${WE} ]]; then
  intent="Fix in code. Never suppress with disable comments."
elif [[ "$prompt_lower" =~ ${WB}(delete|remove|clean\ up)${WE} ]]; then
  intent="Verify no deps before removing."
elif [[ "$prompt_lower" =~ ${WB}(review|audit)${WE} ]]; then
  intent="Review: forbidden syntax, quality gates, test coverage."
fi

# 2b. Scale nudge — independent of the single intent hint above (a task can be
# both a "fix" AND large). Fires only on words that indicate real SCALE, and it
# suggests considering a split rather than instructing a fan-out.
#
# The trigger list used to include `refactor`, `audit` and `across`, which are
# everyday words: "refactor this function", "audit this helper" and "rename it
# across two files" are all single-thread work, and each one was being told to
# delegate and parallelize. Spawning agents for small tasks is slower than doing
# them, so a nudge that fires on ordinary phrasing costs time rather than saving
# it. What remains names a scope no single sweep covers.
#
# WB/WE-wrapped like the other hints to avoid substring false positives.
orchestrate=""
if [[ "$prompt_lower" =~ ${WB}(migrate|codebase-wide|throughout|end-to-end)${WE} ]]; then
  orchestrate="Possibly large task — if it splits into genuinely independent units, consider decomposing it; if it is really one thread of work, just do it. The orchestrator skill has the test for which."
fi

# 2c. Research nudge — independent of the single intent hint (a prompt can be
# both a "fix" AND need external lookup). Fires on EXTERNAL-knowledge signals
# (live docs, latest releases, third-party APIs) so the work is delegated to the
# research-agent subagent — it isolates the token cost and routes
# exa-search/context7 with a native fallback. Deliberately tight and
# external-leaning: words here must NOT overlap the codebase-recall signals
# (`how does`, `where is`) handled by the recall block above, so a local-code
# question never gets misrouted to web research. WB/WE-wrapped. Gated by the
# `agents.research-agent` toggle (default on).
research=""
if toolu_enabled agents research-agent &&
   [[ "$prompt_lower" =~ ${WB}(latest|docs\ for|library\ docs|api\ reference|api\ docs|changelog|release\ notes|best\ practices?|look\ up|search\ the\ web|web\ search|how\ to\ use)${WE} ]]; then
  research="External research — delegate to the research-agent subagent (routes exa-search/context7, native fallback) to keep main context lean."
fi

# 2d. Jira discoverability nudge — independent of the single intent hint. Names
# the jira skill and warns off the Atlassian MCP so the skill wins
# tool-selection when the user mentions Jira (today the model often reaches for
# the Atlassian MCP instead). Fires on the words jira/atlassian, an
# *.atlassian.net/browse/<KEY> link, or an issue key (ABC-123) GATED by a
# jira-context word so bare key-shaped tokens (UTF-8, GPT-4, ISO-8601) don't
# trigger it. The URL and issue-key matches use the ORIGINAL $prompt: the key is
# case-sensitive ([A-Z]) and $prompt_lower has already been lowercased.
jira_nudge=""
if [[ "$prompt_lower" =~ ${WB}(jira|atlassian)${WE} ]] ||
   [[ "$prompt" =~ atlassian[.]net/browse/[A-Z][A-Z0-9]+-[0-9]+ ]] ||
   { [[ "$prompt" =~ [A-Z][A-Z0-9]+-[0-9]+ ]] &&
     [[ "$prompt_lower" =~ ${WB}(ticket|issue|board|sprint|epic|backlog)${WE} ]]; }; then
  jira_nudge="Jira mentioned — use the \`jira\` skill (REST wrapper over jira.sh), NOT the Atlassian MCP."
fi

# 3. Per-project context hook — opt-in. Project may emit any string.
project_ctx=""
PROJECT_CONTEXT="$PROJECT_ROOT/$(toolu_project_dirname)/context.sh"
if [ -n "$PROJECT_ROOT" ] && [ -f "$PROJECT_CONTEXT" ]; then
  # shellcheck disable=SC1091  # path is project-specific; sourced only if present
  project_ctx=$(PROMPT="$prompt" bash "$PROJECT_CONTEXT" 2>/dev/null || true)
fi

# ── Combine and output ───────────────────────────────────────────────────────
parts=()
[[ -n "$recall" ]] && parts+=("$recall")
[[ -n "$intent" ]] && parts+=("$intent")
[[ -n "$orchestrate" ]] && parts+=("$orchestrate")
[[ -n "$research" ]] && parts+=("$research")
[[ -n "$jira_nudge" ]] && parts+=("$jira_nudge")
[[ -n "$project_ctx" ]] && parts+=("$project_ctx")
[[ -n "$quality_gate_hint" ]] && parts+=("$quality_gate_hint")

# Join parts with " | "
context=""
if [[ ${#parts[@]} -gt 0 ]]; then
  context="${parts[0]}"
  for part in "${parts[@]:1}"; do
    context="$context | $part"
  done
fi

if command -v jq >/dev/null 2>&1; then
  jq -n --arg ctx "$context" '{
    "hookSpecificOutput": {
      "hookEventName": "UserPromptSubmit",
      "additionalContext": $ctx
    }
  }'
fi

exit 0
