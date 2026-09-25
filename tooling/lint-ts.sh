#!/usr/bin/env bash
# Type-aware oxlint across every Bun workspace package that owns an .oxlintrc.json,
# plus plugin script trees under plugins/*/ that ship one.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
status=0
while IFS= read -r cfg; do
  dir=$(dirname "$cfg")
  echo "lint:ts: $dir"
  (
    cd "$dir"
    if [ -d src ]; then
      oxlint --type-aware --deny-warnings -c .oxlintrc.json src
    elif [ -d scripts ]; then
      oxlint --type-aware --deny-warnings -c .oxlintrc.json scripts
    else
      echo "lint:ts: no src/ or scripts/ under $dir" >&2
      exit 1
    fi
  ) || status=1
done < <(
  find "$ROOT/tooling" "$ROOT/packages" "$ROOT/tools" "$ROOT/plugins" \
    -name .oxlintrc.json -not -path '*/node_modules/*' \
    -not -path "$ROOT/tools/toolu-opencode/plugins/*" | sort
)
exit "$status"
