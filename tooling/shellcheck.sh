#!/usr/bin/env bash
# Repo-wide shellcheck gate.
#
# Two passes, because the repo has two kinds of shell file:
#
#   1. Standalone scripts — hooks, entrypoints, libs, skill helpers. Linted
#      directly.
#   2. Concern FRAGMENTS (plugins/*/hooks/concerns/NN-*.sh) — partials of one
#      script that register.sh concatenates in numeric order. Linting a fragment
#      on its own is meaningless: it has no shebang (SC2148) and every variable
#      shared with a sibling fragment reads as unassigned (SC2154) or unused
#      (SC2034). This pass assembles each concerns dir exactly the way
#      register.sh does and lints the ASSEMBLED module — i.e. the code that
#      actually runs.
#
# Severity is `warning`: info-level SC2016 ("expressions don't expand in single
# quotes") fires on every jq/awk program in the repo, where single quotes are the
# whole point. Raising the floor keeps the gate meaningful without scattering
# per-line disable comments.
#
# Usage: bash tooling/shellcheck.sh
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v shellcheck >/dev/null 2>&1; then
  echo "shellcheck not on PATH — install it (brew install shellcheck / apt-get install shellcheck)" >&2
  exit 1
fi

SEVERITY="${SHELLCHECK_SEVERITY:-warning}"
status=0

# --- pass 1: standalone scripts -------------------------------------------
standalone=()
while IFS= read -r f; do
  standalone+=("$f")
done < <(find plugins tooling benchmarks .claude-plugin -name '*.sh' \
           -not -path '*/node_modules/*' -not -path '*/concerns/*' | sort)

if [ ${#standalone[@]} -gt 0 ]; then
  echo "shellcheck: ${#standalone[@]} standalone scripts"
  shellcheck -S "$SEVERITY" "${standalone[@]}" || status=1
fi

# --- pass 2: assembled concern modules ------------------------------------
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

while IFS= read -r dir; do
  plugin=$(basename "$(dirname "$(dirname "$dir")")")
  assembled="$tmpdir/${plugin}-assembled.sh"
  : > "$assembled"
  # Same ordering register.sh uses: numeric prefix, lexical.
  for frag in "$dir"/[0-9][0-9]-*.sh; do
    [ -f "$frag" ] || continue
    cat "$frag" >> "$assembled"
  done
  [ -s "$assembled" ] || continue
  echo "shellcheck: assembled $plugin concerns"
  shellcheck -S "$SEVERITY" "$assembled" || status=1
done < <(find plugins -type d -name concerns -not -path '*/node_modules/*' | sort)

if [ "$status" -ne 0 ]; then
  echo "shellcheck: findings above must be fixed in code (no disable comments)" >&2
fi
exit "$status"
