#!/usr/bin/env bash
# Type-aware oxlint across every Bun workspace package that owns an .oxlintrc.json.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
status=0
while IFS= read -r cfg; do
  dir=$(dirname "$cfg")
  echo "lint:ts: $dir"
  (
    cd "$dir"
    oxlint --type-aware --deny-warnings -c .oxlintrc.json src
  ) || status=1
done < <(find "$ROOT/tooling" "$ROOT/packages" "$ROOT/tools" -name .oxlintrc.json -not -path '*/node_modules/*' | sort)
exit "$status"
