#!/usr/bin/env bats
# Cross-plugin register.sh invariants.
#
# The ts-quality/rust-quality byte-parity drift guard already lives in
# plugins/rust-quality/hooks/__tests__/register-sync.bats — this suite covers the
# invariant that applies to EVERY plugin's register.sh instead.

ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"

@test "every plugin register.sh namespaces its registry entries" {
  # The core dispatcher SKIPS any registry module not named
  # "<plugin-spec>__<name>.sh" (fail-closed, see hooks/lib/dispatch.sh). A
  # register.sh that forgets the namespace installs a module that never runs —
  # silently, since "skipped" and "no findings" look identical.
  local f seen=0
  for f in "$ROOT"/plugins/*/hooks/register.sh; do
    [ -f "$f" ] || continue
    seen=$((seen + 1))
    run grep -q 'SPEC=' "$f"
    [ "$status" -eq 0 ]
    run grep -q '${SPEC}__' "$f"
    [ "$status" -eq 0 ]
  done
  # Guard the guard: a bad glob would vacuously pass the loop above.
  [ "$seen" -gt 0 ]
}
