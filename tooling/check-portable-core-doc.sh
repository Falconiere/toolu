#!/usr/bin/env bash
# Verify docs/portable-core.md against the #205 contract checklist.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOC="${PORTABLE_CORE_DOC:-$ROOT/docs/portable-core.md}"
FIXTURE="$ROOT/tooling/fixtures/portable-core/protected-files-pre.json"

fail() {
  printf 'check-portable-core-doc: %s\n' "$1" >&2
  exit 1
}

[ -f "$DOC" ] || fail "missing $DOC"
[ -f "$FIXTURE" ] || fail "missing $FIXTURE"

# Required section headings (markdown ## )
for heading in \
  '## Pins' \
  '## Package boundaries' \
  '## Zod boundary rules' \
  '## Decision contract' \
  '## Event vocabulary' \
  '## Bash bridge protocol' \
  '## Policy split' \
  '## Protected-files gate trace' \
  '## OpenCode interception' \
  '### Capability-results' \
  '## Release blockers'
do
  grep -qF "$heading" "$DOC" || fail "missing heading: $heading"
done

grep -qF 'v2.0.12' "$DOC" || fail "missing CLI pin v2.0.12"
grep -qF '@opencode/plugin@2.0.12' "$DOC" || fail "missing SDK pin"
grep -qF 'protocolVersion' "$DOC" || fail "missing protocolVersion"
grep -qF 'protected-files.sh' "$DOC" || fail "missing protected-files.sh citation"
grep -qF 'gate-mode.sh' "$DOC" || fail "missing gate-mode.sh citation"
grep -qF 'dispatch.sh' "$DOC" || fail "missing dispatch.sh citation"
grep -qF 'tooling/fixtures/portable-core/protected-files-pre.json' "$DOC" || fail "missing fixture citation"
grep -qF 'deny' "$DOC" || fail "missing deny mapping language"

# Classification vocabulary: require all four, forbid extras in the Policy split section
policy_section=$(awk '/^## Policy split/{flag=1;next} /^## /{flag=0} flag' "$DOC")
[ -n "$policy_section" ] || fail "empty Policy split section"
for token in shell-out port-native port-new no-map; do
  printf '%s\n' "$policy_section" | grep -qF "$token" || fail "missing classification token: $token"
done
# Flag a known-invalid token if someone adds it to the vocabulary section
if printf '%s\n' "$policy_section" | grep -qE '`maybe-later`|^\s*-\s*maybe-later\b'; then
  fail "invalid classification token maybe-later"
fi

# Capability-results markers present
grep -qF 'portable-core-capability-results:start' "$DOC" || fail "missing capability-results start marker"
grep -qF 'portable-core-capability-results:end' "$DOC" || fail "missing capability-results end marker"

printf 'check-portable-core-doc: ok\n'
