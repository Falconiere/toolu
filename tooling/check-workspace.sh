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

for p in packages/toolu-core tools/toolu-opencode tools/toolu-conformance tools/toolu-cli tooling; do
  [ -f "$p/package.json" ] || fail "missing $p/package.json"
done

jq -e '.workspaces | index("packages/toolu-core") and index("tools/toolu-opencode") and index("tools/toolu-conformance") and index("tools/toolu-cli")' package.json >/dev/null \
  || fail "package.json workspaces must include tooling, packages/*, tools/*"

# Export smoke via bun module resolution (assert return values)
bun -e 'import { parseDecision } from "@toolu/core/decision"; const d = parseDecision({ kind: "allow" }); if (d.kind !== "allow") throw new Error("decision");'
bun -e 'import { classifyStub } from "@toolu/opencode/plugin-stub"; const r = classifyStub("shell-out"); if (r !== "shell-out") throw new Error("classify");'
bun -e 'import { runProtectedFilesConformance } from "@toolu/conformance/run-stub"; const r = await runProtectedFilesConformance(); if (!r.pass) throw new Error(r.message);'

# CLI smoke: the entry point runs under Bun and reports the workspace version.
cli_ver=$(bun run tools/toolu-cli/src/cli.ts --version)
[ "$cli_ver" = "$(jq -r .version tools/toolu-cli/package.json)" ] \
  || fail "toolu-cli --version reported '$cli_ver'"

echo "check-workspace: ok"
