#!/usr/bin/env bash
# Regenerate plugins/statusline/hooks/pricing-vendored.sh from the canonical
# pricing source (plugins/stats/scripts/lib/pricing.sh).
#
# Why a vendored copy: stats and statusline install under separate
# CLAUDE_PLUGIN_ROOTs, so the statusline hook cannot `source` stats' pricing.sh
# at runtime. It instead sources a byte-identical sibling copy, and this script
# keeps that copy in sync. Run it after ANY rate change in pricing.sh.
#
# Drift between the canonical source and the vendored copy is caught by
# plugins/statusline/__tests__/pricing-sync.bats — a stale copy fails the build.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
src="$repo/plugins/stats/scripts/lib/pricing.sh"
dst="$repo/plugins/statusline/hooks/pricing-vendored.sh"

[ -f "$src" ] || { echo "sync-pricing: missing canonical source $src" >&2; exit 1; }
# shellcheck source=/dev/null
source "$src"

{
  printf '%s\n' '#!/usr/bin/env bash'
  printf '%s\n' '# VENDORED — DO NOT EDIT BY HAND. Byte-identical copy of stats_pricing_jq'
  printf '%s\n' '# from plugins/stats/scripts/lib/pricing.sh. Cross-plugin `source` is'
  printf '%s\n' '# impossible (separate CLAUDE_PLUGIN_ROOTs), so token-ledger.sh sources this'
  printf '%s\n' '# sibling copy. Regenerate: bash plugins/statusline/scripts/sync-pricing.sh'
  printf '%s\n' '# Drift is caught by plugins/statusline/__tests__/pricing-sync.bats.'
  printf '%s\n' '_tl_pricing_jq() {'
  printf '%s\n' "  cat <<'JQ'"
  stats_pricing_jq
  printf '%s\n' 'JQ'
  printf '%s\n' '}'
} > "$dst"

echo "sync-pricing: regenerated $dst"
