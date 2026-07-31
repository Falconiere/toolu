#!/usr/bin/env bats
# Docs-in-sync harness for the workflow-harness-v2 PR (spec component 11,
# AC-12). Asserts the example config, docs/config.md, the push-review v2 docs
# trio, and the 5 workflow SKILL.mds actually mention the new surface — not an
# auto-derivation from the readers, a hand-maintained list that must be kept
# current alongside the config/docs it checks.

bats_require_minimum_version 1.5.0

ROOT="${BATS_TEST_DIRNAME}/../../../.."
EXAMPLE_CFG="$ROOT/plugins/toolu/settings/toolu.config.example.json"
CONFIG_DOC="$ROOT/docs/config.md"
PUSH_REVIEW_DOC="$ROOT/plugins/toolu/hooks/docs/push-review.md"
TOOLU_REVIEW_README="$ROOT/docs/toolu-review/README.md"
PR_BABYSIT_README="$ROOT/docs/pr-babysit/README.md"

# Hand-maintained: every new config key path introduced by workflow-harness-v2
# (spec Interfaces + the already-shipped-but-missing docsSync globs), as a jq
# path expression to probe for presence. Asserted below via `!= null`, which is
# only correct because none of these example-config values are legitimately
# null; a key whose real value could be null/false would need `has`-style path
# presence instead.
REQUIRED_KEY_PATHS=(
  ".telemetry.enabled"
  ".agentTier.mode"
  ".planLedger.blockOnUncoveredAcs"
  ".docsSync.mode"
  ".docsSync.surfaces"
  ".docsSync.surfaceExcludes"
  ".docsSync.codeSurfaces"
  ".lang.ts.noMocks"
  ".lang.rust.noMocks"
)

# Hand-maintained: config key NAMES that must be documented in prose in
# docs/config.md (independent of the example file's jq paths above).
REQUIRED_DOC_NAMES=(
  "telemetry.enabled"
  "agentTier.mode"
  "planLedger.blockOnUncoveredAcs"
  "docsSync.mode"
  "docsSync.surfaces"
  "docsSync.surfaceExcludes"
  "docsSync.codeSurfaces"
  "lang.ts.noMocks"
  "lang.rust.noMocks"
)

VERDICT_SKILLS=(execution execution-review)
DELEGATION_SKILLS=(plan plan-review)

setup() {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
}

@test "example config: file exists and is valid JSON" {
  [ -f "$EXAMPLE_CFG" ]
  jq -e . "$EXAMPLE_CFG" >/dev/null
}

@test "example config: every workflow-harness-v2 key is present" {
  for path in "${REQUIRED_KEY_PATHS[@]}"; do
    jq -e "$path != null" "$EXAMPLE_CFG" >/dev/null \
      || { echo "missing key in $EXAMPLE_CFG: $path" >&2; return 1; }
  done
}

@test "docs/config.md: documents every new key name" {
  [ -f "$CONFIG_DOC" ]
  for name in "${REQUIRED_DOC_NAMES[@]}"; do
    grep -qF "$name" "$CONFIG_DOC" \
      || { echo "docs/config.md does not mention: $name" >&2; return 1; }
  done
}

@test "push-review docs trio: show the v2 schema (reviewed_files + version 2)" {
  for f in "$PUSH_REVIEW_DOC" "$TOOLU_REVIEW_README" "$PR_BABYSIT_README"; do
    [ -f "$f" ]
    grep -q 'reviewed_files' "$f" \
      || { echo "$f: no mention of reviewed_files" >&2; return 1; }
    grep -qi 'version 2' "$f" \
      || { echo "$f: no mention of schema version 2" >&2; return 1; }
    # No lingering v1-only schema presentation (the pre-reviewed_files shape).
    ! grep -q '"version": 1,' "$f" \
      || { echo "$f: still shows a version-1 push-review schema block" >&2; return 1; }
  done
}

@test "execution + execution-review SKILL.mds: mention the verdict command" {
  for skill in "${VERDICT_SKILLS[@]}"; do
    f="$ROOT/plugins/toolu/skills/$skill/SKILL.md"
    [ -f "$f" ]
    grep -q 'verdict.sh' "$f" \
      || { echo "$f: does not mention verdict.sh" >&2; return 1; }
  done
}

@test "execution-review SKILL.md: names the reviewed_files v2 requirement" {
  f="$ROOT/plugins/toolu/skills/execution-review/SKILL.md"
  grep -q 'reviewed_files' "$f"
}

@test "plan + plan-review SKILL.mds: mention delegation telemetry for model tiers" {
  for skill in "${DELEGATION_SKILLS[@]}"; do
    f="$ROOT/plugins/toolu/skills/$skill/SKILL.md"
    [ -f "$f" ]
    grep -qi 'delegation' "$f" \
      || { echo "$f: does not mention delegation telemetry" >&2; return 1; }
  done
}

@test "test SKILL.md: the no-mocks rule is named as mechanically enforced" {
  f="$ROOT/plugins/toolu/skills/test/SKILL.md"
  grep -qi 'no-mocks.sh' "$f"
}
