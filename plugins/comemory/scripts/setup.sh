#!/usr/bin/env bash
# /comemory:setup — detect-and-guide the comemory binary, then wire repo-local
# memory + code indexing for first-time use.
#
# This script NEVER runs a package manager. When the binary is absent or below
# the version floor it prints the canonical install command and stops — the user
# runs it. Output begins with a STATUS token the /comemory:setup command reads:
#   READY   binary present and >= floor; phase-2 wiring ran (details follow)
#   MISSING binary not on PATH; install hint printed; stop
#   OLD     binary below the floor; upgrade hint printed; stop
#   ERROR   unexpected (could not parse the version); stop non-zero
#
# Phase 2 (only when READY; each step best-effort + non-fatal):
#   data dir -> repo scope -> install-hooks (git auto-index) -> initial
#   index-code -> shell-completions hint.
#
# The COMEMORY override is a test seam: `COMEMORY=/nonexistent bash setup.sh`
# drives the MISSING branch without stripping coreutils off PATH.
#
# No `set -e`: phase-2 steps are intentionally non-fatal and each handles its own
# exit status, so one failure never aborts the rest.
set -uo pipefail

COMEMORY="${COMEMORY:-comemory}"

# Mirror of toolu's COMEMORY_MIN_VERSION (plugins/toolu/hooks/lib/detect.sh).
# That lib is in a sibling plugin with no stable runtime path from here, so the
# value is mirrored, not sourced; a version-const guard test fails CI if the two
# drift. Same constant name in both files so the guard can match by name.
COMEMORY_MIN_VERSION="0.8.0"

# Shared repo-scope helpers (comemory_repo_key, version_ge) — one definition for
# all three comemory entry points (this lib ships with the plugin, so the sibling
# path is stable at runtime).
_rs="${BASH_SOURCE%/*}/../lib/repo-scope.sh"
# shellcheck source=../lib/repo-scope.sh
. "$_rs" || { printf 'setup.sh: cannot load %s\n' "$_rs" >&2; exit 1; }

# Canonical install (comemory is NOT published to crates.io — use the tap).
BREW_INSTALL="brew install Falconiere/tap/comemory"
BREW_UPGRADE="brew upgrade Falconiere/tap/comemory"
CURL_INSTALL="curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Falconiere/comemory/releases/latest/download/comemory-installer.sh | sh"

say() { printf '%s\n' "$*"; }

print_usage() {
  cat <<'USAGE'
Usage: setup.sh [--force] [-h|--help]

Detect the comemory binary (install-guide if absent/old), then wire git
index-code hooks, an initial code index, the data dir, and a completions hint
for the current repo. --force is forwarded to `comemory install-hooks` to
overwrite pre-existing git hooks.
USAGE
}

force=0
for a in "$@"; do
  case "$a" in
    --force | -f) force=1 ;;
    -h | --help)  print_usage; exit 0 ;;
    *)            say "ERROR unknown argument: $a"; print_usage; exit 1 ;;
  esac
done

# ── Phase 1 — binary gate ────────────────────────────────────────────────────
if ! command -v "$COMEMORY" >/dev/null 2>&1; then
  say "MISSING comemory CLI not found on PATH."
  say "Install (comemory is not on crates.io — use the Homebrew tap):"
  say "    $BREW_INSTALL"
  say "Or the curl installer:"
  say "    $CURL_INSTALL"
  say "Re-run /comemory:setup after installing."
  exit 0
fi

ver=$("$COMEMORY" --version 2>/dev/null | awk '{print $2}')
if [ -z "$ver" ]; then
  say "ERROR could not parse '$COMEMORY --version'."
  exit 1
fi

if ! version_ge "$ver" "$COMEMORY_MIN_VERSION"; then
  say "OLD comemory $ver is below the v$COMEMORY_MIN_VERSION floor toolu targets."
  say "Upgrade:"
  say "    $BREW_UPGRADE"
  exit 0
fi

say "READY comemory $ver (>= $COMEMORY_MIN_VERSION). Wiring repo-local memory + indexing:"

# ── Phase 2 — wiring (non-fatal) ─────────────────────────────────────────────
data_dir="${COMEMORY_DATA_DIR:-$HOME/.comemory}"
if mkdir -p "$data_dir" 2>/dev/null; then
  say "  data-dir: $data_dir"
else
  say "  data-dir: WARN could not create $data_dir"
fi

scope=$(comemory_repo_key)
if [ -n "$scope" ]; then
  say "  repo scope: $scope"
else
  say "  repo scope: WARN not in a git repo — memory scopes to 'unknown' (set MY_CLAUDE_COMEMORY_REPO)"
fi

# git hooks: auto-refresh the code index on commit/merge/checkout. No --force by
# default — the CLI refuses to clobber hand-written hooks; surface that refusal.
if [ -n "$scope" ]; then
  fflag=()
  [ "$force" = 1 ] && fflag=(--force)
  if hooks_out=$("$COMEMORY" install-hooks ${fflag[@]+"${fflag[@]}"} 2>&1); then
    say "  install-hooks: OK (post-commit/merge/checkout)"
  else
    say "  install-hooks: skipped (see detail) — if it refused to clobber existing hooks, re-run '/comemory:setup --force':"
    printf '%s\n' "$hooks_out" | sed 's/^/      /'
  fi
else
  say "  install-hooks: skipped (not a git repo)"
fi

# Initial code index so search-code works immediately and 0.9.0 auto-reinforcement
# has a baseline. Bounded (can be slow on big repos) and non-fatal.
if [ -n "$scope" ]; then
  to=""
  command -v timeout  >/dev/null 2>&1 && to="timeout 120"
  command -v gtimeout >/dev/null 2>&1 && to="gtimeout 120"
  if $to "$COMEMORY" index-code --repo "$scope" --path . >/dev/null 2>&1; then
    say "  index-code: OK (repo $scope)"
  else
    say "  index-code: skipped/slow (non-fatal) — run later: comemory index-code --repo $scope --path ."
  fi
else
  say "  index-code: skipped (not a git repo)"
fi

# Opt-in marker — record that setup ran so toolu treats comemory as enabled for
# this repo. Writes ONLY a JSON config (no package manager). The target path
# mirrors toolu's _toolu_project_cfg EXACTLY (config.sh is in a sibling plugin,
# barred from sourcing per the header — so the path logic is replicated inline,
# NOT comemory_repo_key, which returns a basename / the main-repo path).
m_root="${TOOLU_PROJECT_DIR:-${CLAUDE_PROJECT_DIR:-}}"
[ -z "$m_root" ] && m_root=$(git rev-parse --show-toplevel 2>/dev/null || true)
m_dir="${TOOLU_PROJECT_CONFIG_DIRNAME:-.claude}"
if [ -z "$m_root" ]; then
  say "  memory: WARN not in a git repo (no TOOLU_PROJECT_DIR/CLAUDE_PROJECT_DIR) — skipped setup_done marker"
elif ! command -v jq >/dev/null 2>&1; then
  say "  memory: WARN jq unavailable — skipped setup_done marker"
else
  m_target="$m_root/$m_dir/toolu.config.json"
  # jq-merge into the existing config (or {}) so all other keys survive; write
  # atomically via a tmp file in the same dir + mv. Guard first: a pre-existing
  # target that is NOT valid JSON would fail the merge with a generic WARN and
  # leave the marker unwritten (opt-in silently never enables). Name the real
  # cause and skip — never overwrite the user's file (no data loss).
  if [ -f "$m_target" ] && ! jq -e . "$m_target" >/dev/null 2>&1; then
    say "  memory: WARN $m_target is not valid JSON — fix it, then re-run /comemory:setup"
  else
    m_existing="{}"
    [ -f "$m_target" ] && m_existing=$(cat "$m_target")
    m_tmp="$m_target.tmp.$$"
    if mkdir -p "$m_root/$m_dir" 2>/dev/null \
       && printf '%s' "$m_existing" | jq '.comemory.setup_done = true' > "$m_tmp" 2>/dev/null \
       && mv "$m_tmp" "$m_target" 2>/dev/null; then
      say "  memory: enabled (setup_done marker written: $m_target)"
    else
      rm -f "$m_tmp" 2>/dev/null
      say "  memory: WARN could not write setup_done marker to $m_target"
    fi
  fi
fi

# Shell completions — print the install one-liner for the detected shell (hint
# only, no file write; consistent with the detect+guide stance).
sh_name=$(basename "${SHELL:-}")
case "$sh_name" in
  bash | zsh | fish)
    say "  completions: comemory completions $sh_name  (target path: comemory completions --help)" ;;
  *)
    say "  completions: comemory completions <bash|zsh|fish|powershell|elvish>" ;;
esac

say "Setup complete."
exit 0
