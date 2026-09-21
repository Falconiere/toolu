#!/usr/bin/env bats
# Conventions guardrails (#213) — real tools against fixture trees.

setup() {
  ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  FIX="$ROOT/tooling/fixtures/conventions"
  GR="$ROOT/tooling/conventions/guardrails/run.sh"
  OXLINT="$ROOT/node_modules/.bin/oxlint"
}

@test "adoption docs record pin, thresholds, and Lefthook note" {
  grep -Fq '3562c63' "$ROOT/tooling/conventions/PROVENANCE.md"
  grep -Eq '300' "$ROOT/docs/conventions-adoption.md"
  grep -Eq '60' "$ROOT/docs/conventions-adoption.md"
  grep -Fq 'Lefthook' "$ROOT/docs/conventions-adoption.md"
}

@test "package.json wires test:conventions and zod without yup" {
  jq -e '.scripts["test:conventions"] and (.scripts.test | contains("test:conventions"))' "$ROOT/package.json"
  jq -e '.dependencies.zod and (.dependencies|has("yup")|not)' "$ROOT/package.json"
}

@test "root workspace passes banned-deps" {
  run bash "$GR" --only banned-deps
  [ "$status" -eq 0 ]
}

@test "violating fixture with valibot fails banned-deps in temp tree" {
  local tmp="$BATS_TEST_TMPDIR/violate-deps"
  mkdir -p "$tmp/src/utilities"
  cp "$FIX/violating/package.json" "$tmp/package.json"
  cp "$FIX/violating/guardrails.config.json" "$tmp/guardrails.config.json"
  cp "$FIX/clean/src/utilities/parse-id.ts" "$tmp/src/utilities/parse-id.ts"
  cd "$tmp"
  run bash "$GR" --only banned-deps
  [ "$status" -ne 0 ]
}

@test "oxlint fails on explicit any and type assertion fixture" {
  run "$OXLINT" -c "$FIX/oxlint.fixture.json" --deny-warnings \
    "$FIX/violating/src/utilities/bad.ts"
  [ "$status" -ne 0 ]
  printf '%s\n' "$output" "$stderr" | grep -Eiq 'any|assertion|explicit|typescript'
}

@test "production tooling sources have no type assertions" {
  if grep -R --include='*.ts' -nE ' as [A-Za-z{]' "$ROOT/tooling/src" | grep -v ' as const'; then
    return 1
  fi
  return 0
}

@test "lint-suppressions --file fails on unused-vars disable" {
  local tmp="$BATS_TEST_TMPDIR/suppress"
  mkdir -p "$tmp/src/utilities"
  cp "$FIX/clean/guardrails.config.json" "$tmp/guardrails.config.json"
  cat > "$tmp/package.json" <<'EOF'
{ "name": "suppress-fixture", "private": true }
EOF
  cat > "$tmp/src/utilities/suppressed.ts" <<'EOF'
/** Intentional unused-vars suppression for fixture. */
// oxlint-disable-next-line eslint/no-unused-vars
const suppressed = 1;
export function ok(): number {
  return 1;
}
EOF
  cd "$tmp"
  run bash "$GR" --only lint-suppressions --file src/utilities/suppressed.ts
  [ "$status" -ne 0 ]
  printf '%s\n' "$output" "$stderr" | grep -Fq 'lint-suppressions'
}
