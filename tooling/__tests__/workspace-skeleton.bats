#!/usr/bin/env bats
# Bun workspace skeleton (#208)

setup() {
  ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
}

@test "workspaces list core, opencode, and conformance packages" {
  jq -e '
    (.workspaces | index("packages/toolu-core")) and
    (.workspaces | index("tools/toolu-opencode")) and
    (.workspaces | index("tools/toolu-conformance"))
  ' "$ROOT/package.json"
  test -f "$ROOT/packages/toolu-core/package.json"
  test -f "$ROOT/tools/toolu-opencode/package.json"
  test -f "$ROOT/tools/toolu-conformance/package.json"
  test -d "$ROOT/packages/toolu-core/src"
  test -d "$ROOT/tools/toolu-opencode/src"
  test -d "$ROOT/tools/toolu-conformance/src"
}

@test "CI workflow defines a typescript job running test:ts" {
  run awk '
    /^  typescript:/ { in_job=1; found_job=1; next }
    /^  [a-z]/ && $0 !~ /^    / { if ($0 !~ /^  typescript:/) in_job=0 }
    in_job && /bun run test:ts/ { found_run=1 }
    END { if (!(found_job && found_run)) { print "missing typescript job or bun run test:ts"; exit 1 } }
  ' "$ROOT/.github/workflows/tests.yml"
  [ "$status" -eq 0 ]
}

@test "root test script includes test:ts" {
  jq -e '.scripts["test:ts"] and (.scripts.test | contains("test:ts"))' "$ROOT/package.json"
}

@test "portable-core documents tools/toolu-conformance" {
  grep -Fq 'tools/toolu-conformance' "$ROOT/docs/portable-core.md"
}

@test "conventions adoption documents local TS CI commands" {
  grep -Eq 'test:ts|typescript' "$ROOT/docs/conventions-adoption.md"
}
