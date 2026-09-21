#!/usr/bin/env bash
# Workspace smoke: required packages exist, exports resolve, Bun version pin holds.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

fail() { echo "check-workspace: $*" >&2; exit 1; }

bun_ver=$(bun --version)
case "$bun_ver" in
  1.4.*) ;;
  *) fail "Bun $bun_ver not in supported 1.4.x range (docs/portable-core.md)" ;;
esac

for p in packages/toolu-core tools/toolu-opencode tools/toolu-conformance tooling; do
  [ -f "$p/package.json" ] || fail "missing $p/package.json"
done

jq -e '.workspaces | index("packages/toolu-core") and index("tools/toolu-opencode") and index("tools/toolu-conformance")' package.json >/dev/null \
  || fail "package.json workspaces must include tooling, packages/*, tools/*"

# Export smoke via bun module resolution
bun -e 'import { parseDecision } from "@toolu/core/decision"; parseDecision({ kind: "allow" });'
bun -e 'import { classifyStub } from "@toolu/opencode/plugin-stub"; classifyStub("shell-out");'
bun -e 'import { checkConfigStub } from "@toolu/conformance/run-stub"; checkConfigStub({ version: 1 });'

echo "check-workspace: ok"
