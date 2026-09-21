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
  grep -Eq '^[[:space:]]*typescript:' "$ROOT/.github/workflows/tests.yml"
  grep -Fq 'bun run test:ts' "$ROOT/.github/workflows/tests.yml"
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
