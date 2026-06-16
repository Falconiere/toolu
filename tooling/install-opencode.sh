#!/usr/bin/env bash
# Install the toolu bundle into a target opencode project.
#
# Default: install into the CURRENT directory (project install).
# --global: install into ~/.config/opencode/ (user install).
#
# What gets installed:
#   1. The opencode adapter (opencode/extensions/toolu.ts)
#      → <target>/.opencode/plugins/toolu.ts
#   2. opencode-format agents (opencode/agents/*.md)
#      → <target>/.opencode/agents/<name>.md
#   3. opencode-format commands (opencode/commands/*.md)
#      → <target>/.opencode/commands/<name>.md
#   4. skills.paths wiring in <target>/opencode.json so opencode can find
#      every plugins/*/skills/*/SKILL.md (opencode only auto-discovers
#      .opencode/skills/, .claude/skills/, and .agents/skills/ by default).
#   5. Each toolu plugin's register.sh, run once to populate the opencode
#      runtime registry under $TOOLU_CONFIG_DIR/toolu/{pre,post}-tools.d/
#
# Re-runnable: every copy uses content-hash skipping, and opencode.json
# is updated in place (never clobbered) via jq merge. Existing files at
# <target>/.opencode/ are preserved unless they were previously installed
# by this script.
#
# Requires: bash 3.2+, jq, coreutils. macOS stock /bin/bash is fine.

set -euo pipefail

# ── Resolve script-relative paths ───────────────────────────────────────────
# Honor TOOLU_ROOT from the env so tests can exercise paths the script
# does not itself live under (paths containing single quotes, command
# substitutions, etc.). Production callers always invoke the script from
# inside the toolu checkout, where the default resolution is correct.
if [ -z "${TOOLU_ROOT:-}" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  TOOLU_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
fi

# ── Parse args ──────────────────────────────────────────────────────────────
MODE="project"
TARGET=""
while [ $# -gt 0 ]; do
  case "$1" in
    --global)
      MODE="global"
      shift
      ;;
    --target)
      TARGET="${2:-}"
      [ -n "$TARGET" ] || { printf 'install-opencode: --target requires a path\n' >&2; exit 2; }
      shift 2
      ;;
    -h|--help)
      sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      printf 'install-opencode: unknown arg %s\n' "$1" >&2
      exit 2
      ;;
  esac
done

if [ "$MODE" = "global" ]; then
  TARGET="${TARGET:-$HOME/.config/opencode}"
else
  TARGET="${TARGET:-$PWD}"
fi

case "$TARGET" in
  /*) ;;
  *)  TARGET="$PWD/$TARGET" ;;
esac

# ── Preflight ───────────────────────────────────────────────────────────────
command -v jq >/dev/null 2>&1 || {
  printf 'install-opencode: jq is required (https://jqlang.org)\n' >&2
  exit 1
}

[ -d "$TOOLU_ROOT/opencode" ] || {
  printf 'install-opencode: missing opencode/ source dir at %s\n' "$TOOLU_ROOT" >&2
  exit 1
}
[ -d "$TOOLU_ROOT/plugins" ] || {
  printf 'install-opencode: missing plugins/ source dir at %s\n' "$TOOLU_ROOT" >&2
  exit 1
}

printf 'install-opencode: target=%s (%s install)\n' "$TARGET" "$MODE"

# ── 1. Drop the adapter ─────────────────────────────────────────────────────
mkdir -p "$TARGET/.opencode/plugins"
ADAPTER_SRC="$TOOLU_ROOT/opencode/extensions/toolu.ts"
ADAPTER_DST="$TARGET/.opencode/plugins/toolu.ts"
if [ ! -f "$ADAPTER_DST" ] || ! cmp -s "$ADAPTER_SRC" "$ADAPTER_DST"; then
  cp "$ADAPTER_SRC" "$ADAPTER_DST"
  printf 'install-opencode:   ✓ adapter  → %s\n' "${ADAPTER_DST#"$TARGET"/}"
else
  printf 'install-opencode:   · adapter  up to date\n'
fi

# ── 2. Copy opencode-format agents ──────────────────────────────────────────
mkdir -p "$TARGET/.opencode/agents"
for src in "$TOOLU_ROOT"/opencode/agents/*.md; do
  [ -f "$src" ] || continue
  name=$(basename "$src")
  dst="$TARGET/.opencode/agents/$name"
  if [ ! -f "$dst" ] || ! cmp -s "$src" "$dst"; then
    cp "$src" "$dst"
    printf 'install-opencode:   ✓ agent    → %s\n' "${dst#"$TARGET"/}"
  fi
done

# ── 3. Copy opencode-format commands ────────────────────────────────────────
mkdir -p "$TARGET/.opencode/commands"
for src in "$TOOLU_ROOT"/opencode/commands/*.md; do
  [ -f "$src" ] || continue
  name=$(basename "$src")
  dst="$TARGET/.opencode/commands/$name"
  if [ ! -f "$dst" ] || ! cmp -s "$src" "$dst"; then
    cp "$src" "$dst"
    printf 'install-opencode:   ✓ command  → %s\n' "${dst#"$TARGET"/}"
  fi
done

# ── 4. Wire skills.paths + $schema in opencode.json ─────────────────────────
CFG="$TARGET/opencode.json"
# Build the skills.paths entry once via --arg so the value is never subject
# to shell or jq string interpolation. A TOOLU_ROOT containing a single
# quote, double quote, backtick, or $() would otherwise be executed or
# break the JSON; --arg treats the value as opaque.
TOOLU_PLUGINS_PATH="$TOOLU_ROOT/plugins"
if [ -f "$CFG" ]; then
  # Preserve the user's existing config: only add keys we own. We do NOT
  # touch any other field. `jq` may re-emit with different whitespace, so
  # we compare semantically (jq -S) rather than byte-for-byte.
  tmp=$(mktemp "${TMPDIR:-/tmp}/install-opencode.XXXXXX")
  if ! jq -S --arg plugins "$TOOLU_PLUGINS_PATH" '
    .["$schema"] //= "https://opencode.ai/config.json"
    | .skills //= {}
    | .skills.paths //= []
    | .skills.paths = (([$plugins] + .skills.paths) | unique)
  ' "$CFG" > "$tmp"; then
    rm -f "$tmp"
    printf 'install-opencode: failed to merge into %s (check syntax)\n' "$CFG" >&2
    exit 1
  fi
  if jq -e . "$CFG" >/dev/null 2>&1 && jq -e . "$tmp" >/dev/null 2>&1; then
    if [ "$(jq -S . "$CFG")" = "$(jq -S . "$tmp")" ]; then
      rm -f "$tmp"
      printf 'install-opencode:   · config   up to date\n'
    else
      mv "$tmp" "$CFG"
      printf 'install-opencode:   ✓ config   → %s (skills.paths merged)\n' "${CFG#"$TARGET"/}"
    fi
  else
    # Either file is unparseable — bail and let the user fix it by hand.
    rm -f "$tmp"
    printf 'install-opencode: failed to compare %s (unparseable JSON)\n' "$CFG" >&2
    exit 1
  fi
else
  # Create the new file via jq -n so the path value is treated as data, not
  # subject to bash heredoc expansion. The previous <<JSON heredoc would
  # have executed $(...) or backticks in a maliciously-named TOOLU_ROOT.
  if ! jq -n --arg plugins "$TOOLU_PLUGINS_PATH" '
    { "$schema": "https://opencode.ai/config.json", skills: { paths: [ $plugins ] } }
  ' > "$CFG"; then
    printf 'install-opencode: failed to create %s\n' "$CFG" >&2
    exit 1
  fi
  printf 'install-opencode:   ✓ config   → %s (created)\n' "${CFG#"$TARGET"/}"
fi

# ── 5. Run every plugin's register.sh to populate the opencode registry ────
# This is the same call the opencode adapter makes on session.created. We
# run it once at install time so the user can verify the registry looks
# correct before the first session.
AGENT_DIR="$TOOLU_CONFIG_DIR"
if [ -z "$AGENT_DIR" ]; then
  if [ "$MODE" = "global" ]; then
    AGENT_DIR="$TARGET"
  else
    AGENT_DIR="$HOME/.config/opencode"
  fi
fi
export TOOLU_CONFIG_DIR="$AGENT_DIR"
export OPENCODE_CONFIG_DIR="$AGENT_DIR"
export TOOLU_RUNTIME="opencode"
export TOOLU_PROJECT_CONFIG_DIRNAME=".opencode"
mkdir -p "$AGENT_DIR/toolu/pre-tools.d" "$AGENT_DIR/toolu/post-tools.d"

printf 'install-opencode:   · registry sync → %s\n' "$AGENT_DIR/toolu"
for register in \
  "$TOOLU_ROOT/plugins/ast-grep/hooks/register.sh" \
  "$TOOLU_ROOT/plugins/comemory/hooks/register.sh" \
  "$TOOLU_ROOT/plugins/ts-quality/hooks/register.sh" \
  "$TOOLU_ROOT/plugins/rust-quality/hooks/register.sh" \
  "$TOOLU_ROOT/plugins/git-better/hooks/register.sh"; do
  if [ -f "$register" ]; then
    if ! bash "$register" </dev/null; then
      printf 'install-opencode:   ! register failed: %s (non-fatal; session.created will retry)\n' "${register#"$TOOLU_ROOT"/}" >&2
    fi
  fi
done

# ── 6. Sanity check + summary ───────────────────────────────────────────────
adapters_present=0
[ -f "$ADAPTER_DST" ] && adapters_present=1
agents_count=$(find "$TARGET/.opencode/agents" -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' ')
commands_count=$(find "$TARGET/.opencode/commands" -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' ')
registry_count=$(find "$AGENT_DIR/toolu" -maxdepth 2 -name '*.sh' 2>/dev/null | wc -l | tr -d ' ')

printf '\ninstall-opencode: done.\n'
printf '  adapter   : %s\n' "$([ "$adapters_present" -eq 1 ] && echo 'installed' || echo 'MISSING')"
printf '  agents    : %s file(s) in .opencode/agents/\n' "$agents_count"
printf '  commands  : %s file(s) in .opencode/commands/\n' "$commands_count"
printf '  registry  : %s hook module(s) in %s/toolu/\n' "$registry_count" "$AGENT_DIR"
printf '\nNext: restart opencode so the plugin and skills.paths take effect.\n'

if [ "$agents_count" -eq 0 ] || [ "$commands_count" -eq 0 ] || [ "$adapters_present" -eq 0 ]; then
  printf 'install-opencode: WARN — one or more components missing above; the bundle is incomplete.\n' >&2
  exit 1
fi
