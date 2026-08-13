#!/usr/bin/env bash
# Post-tool check: Quality gate state tracking
# Persists global gate state to the host-native project state directory based on
# the exit code of recognized quality/test commands.
#
# Inputs (from parent dispatcher post-tools/mod.sh, via `export`):
#   $tool_name     - name of the tool being invoked
#   $input         - raw JSON payload on stdin
#   $PROJECT_ROOT  - repository root

: "${tool_name:=}"
: "${input:=}"
: "${PROJECT_ROOT:=$(pwd)}"

# Cursor Agent uses tool_name "Shell"; Claude Code uses "Bash".
[[ "$tool_name" != "Bash" && "$tool_name" != "Shell" ]] && exit 0

_toolu_lib="${TOOLU_LIB_DIR:-${BASH_SOURCE%/*}/../../lib}"
# shellcheck source=../../lib/host.sh
. "$_toolu_lib/host.sh"
GATE_DIR="$(toolu_project_state_root "$PROJECT_ROOT")"
GATE_FILE="$GATE_DIR/quality-gate-status.json"
mkdir -p "$GATE_DIR"

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null || echo "")
exit_code=$(echo "$input" | jq -r '.tool_response.metadata.exit_code // .tool_response.exit_code // .tool_output.exit_code // empty' 2>/dev/null || echo "")
if [[ -z "$exit_code" || "$exit_code" == "null" ]]; then
  tool_output_raw=$(echo "$input" | jq -r '.tool_output // empty' 2>/dev/null || echo "")
  if [[ -n "$tool_output_raw" ]]; then
    exit_code=$(echo "$tool_output_raw" | jq -r '.exitCode // .exit_code // empty' 2>/dev/null || echo "")
  fi
fi

# A failing gate owned by a file-level quality hook (ts-quality-hook,
# rust-quality-hook, ...) is NOT clobbered by what follows: this module records
# and clears only its OWN command-channel slot (key "__global__", source
# "gate-status-hook") through gate-file.sh's ownership-checked helpers, so a file
# hook's entry — owned by a different source — survives until that file is fixed.
# This replaces an earlier top-level `source == *-quality-hook` guard that, while
# a file failure was live, dropped a failing command outright; fixing the file
# then cleared the only tracked entry and flipped the gate to passing even though
# the command was still failing.

# Quality / test commands that should toggle the global gate:
#   * Project tool wrappers: tools/<name>/{check,test,format}.sh (any project basename)
#   * TS/JS: bun run script aliases, bun test, vitest, jest, tsc, ./scripts/ts-check.sh
#   * Rust: cargo clippy/test/build/nextest
# The wrapper-path regex is project-agnostic; per-project naming lives in the
# wrapper script, not here.
#
# Anchor every alternative at a COMMAND boundary so substring false positives
# (`cat tsconfig.json` → `tsc`, `vitests-helper` → `vitest`) no longer flip the
# gate. Unlike quality-gate.sh (which only anchors at start-of-line, inspecting
# the leading command of a push), gate-status must still detect a quality
# command that appears AFTER a shell operator in a compound command
# (`cd crate && cargo test`), so the prefix also matches after a separator.
# The trailing `|` (single pipe) in this alternation is BY DESIGN: a quality
# command after a pipe (`find . | cargo test`) still ran, so it must trigger
# gate registration. Do not drop it. (Locked in by gate-status.bats.)
GATE_TRIGGER_PREFIX='(^|[[:space:]]|&&|\|\||;|\|)[[:space:]]*'
GATE_TRIGGER_SUFFIX='([[:space:]]|$|&&|\|\||;|\|)'
GATE_TRIGGER_ALTERNATION='tools/[A-Za-z0-9_.-]+/(check|test|format)\.sh|bun run (check|check:fix|check:duplication|ts:check|ts:check:fix|rust:check|rust:test|check-types|lint|lint:fix|format|format:check|format:fix|build|test)|bun test|vitest|jest|tsc|\./scripts/ts-check\.sh|cargo (clippy|test|build|nextest)'
if ! echo "$command" | grep -qE "${GATE_TRIGGER_PREFIX}(${GATE_TRIGGER_ALTERNATION})${GATE_TRIGGER_SUFFIX}"; then
  exit 0
fi

# Multi-slot gate writer/clearer (entries keyed by file/channel) so a command
# failure no longer clobbers a file hook's failure and vice-versa. Soft if
# absent: the fallbacks keep legacy single-slot behavior when the toolu lib
# predates gate-file.sh.
# shellcheck source=../../lib/gate-file.sh
[ -f "$_toolu_lib/gate-file.sh" ] && . "$_toolu_lib/gate-file.sh"
command -v gate_record_failure >/dev/null 2>&1 || gate_record_failure() {
  jq -n --arg reason "$4" --arg source "$3" --arg file "$2" --arg violations "$5" \
    --arg updatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{status: "failing", reason: $reason, source: $source, file: $file,
      violations: $violations, updatedAt: $updatedAt}' > "$1"
}
command -v gate_clear_file >/dev/null 2>&1 || gate_clear_file() {
  [ -f "$1" ] || return 0
  local _src _file
  _src=$(jq -r '.source // ""' "$1" 2>/dev/null || echo "")
  _file=$(jq -r '.file // ""' "$1" 2>/dev/null || echo "")
  if [ "$_src" = "$3" ] && [ "$_file" = "$2" ]; then
    jq -n --arg source "$3" --arg updatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      '{status: "passing", source: $source, updatedAt: $updatedAt}' > "$1"
  fi
}

# The quality-command gate slot. "__global__" is the same key gate-file.sh
# assigns a no-`file` command failure when it seeds a legacy record, so a record
# written here and a promoted legacy command failure occupy one slot.
GATE_COMMAND_KEY="__global__"

if [[ "$exit_code" =~ ^[0-9]+$ && "$exit_code" -ne 0 ]]; then
  # Record the command failure as its OWN slot. When a file hook's failure is
  # also live this coexists with it (different source/key); fixing the file later
  # promotes this slot instead of flipping the gate to passing while the command
  # is still broken.
  gate_record_failure "$GATE_FILE" "$GATE_COMMAND_KEY" "gate-status-hook" \
    "Quality command failed: $command (exit $exit_code)" ""

  jq -n --arg ctx "Global quality gate failing. Fix all errors/warnings/tests before new tasks.\nFailed: $command (exit $exit_code)" '{
    "hookSpecificOutput": {
      "hookEventName": "PostToolUse",
      "additionalContext": $ctx
    }
  }'
  exit 0
fi

if [[ "$exit_code" == "0" ]]; then
  # Clear ONLY this command-channel slot. gate_clear_file is ownership-checked: a
  # live *-quality-hook file entry is owned by a different source, so it is left
  # intact and the gate stays failing until that file is fixed too.
  gate_clear_file "$GATE_FILE" "$GATE_COMMAND_KEY" "gate-status-hook"

  # Assert an affirmative passing record ONLY when no gate file exists yet — the
  # first-ever passing command, with nothing tracked. In every other case
  # gate_clear_file already did the right thing: it flips the gate to passing when
  # our slot was the last entry, and leaves a live file-hook failure (owned by a
  # different source) untouched. Gating on `! -f` alone means we never re-write a
  # file the helper just wrote (no hot-path double-write) and never clobber a
  # foreign failure (the file is present, so we skip).
  # CONCURRENCY: this write shares gate-file.sh's single-writer assumption — safe
  # only while PostToolUse hooks fire serially; see the CONCURRENCY note there.
  if [ ! -f "$GATE_FILE" ]; then
    jq -n \
      --arg status "passing" \
      --arg source "$command" \
      --arg updatedAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      '{
        status: $status,
        source: $source,
        updatedAt: $updatedAt
      }' > "$GATE_FILE"
  fi
fi

exit 0
