# Release install.sh asset — Plan

**Date:** 2026-10-26   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-26-release-install-asset-design.md   **Topic:** Upload `install.sh` with every release, enforce it in `check-workflows`, and verify the served installer in `/deploy`.

## Evidence and approach

`crates/xtask/src/workflow_install.rs` `signing()` already requires that `release-native.yml` uploads `dist/SHA256SUMS.minisig`, and its unit test lists the messages an empty root must produce. The new requirement follows the same pattern. `release-native.yml`'s `package` job checks out the tag, so `install.sh` is the tagged version, and the `upload` round trip already compares every file under `dist/`. The skill reuses `$D` and its existing round trip.

## Workstream summary

1. Contract check, red then green.
2. Workflow.
3. Skill.
4. Docs.
5. Gate.

## Steps (machine-readable)

```json
[
  {
    "id": "contract-check",
    "title": "check-workflows requires release-native.yml to upload install.sh",
    "check": "cargo test -p xtask workflow_install::tests",
    "ac_refs": ["AC-1"],
    "paths": ["crates/xtask/src/workflow_install.rs", "crates/xtask/src/tests/workflow_install_test.rs"],
    "input": "An empty temp root, which has no workflows.",
    "model": "inherit"
  },
  {
    "id": "workflow",
    "title": "release-native.yml packages and uploads install.sh",
    "check": "cargo xtask check-workflows && grep -qF 'cp install.sh dist/install.sh' .github/workflows/release-native.yml && grep -qF 'dist/install.sh' .github/workflows/release-native.yml",
    "ac_refs": ["AC-1", "AC-2"],
    "depends_on": ["contract-check"],
    "paths": [".github/workflows/release-native.yml", "crates/xtask/src/workflow_install.rs"],
    "input": "This repository's real workflows.",
    "model": "inherit"
  },
  {
    "id": "skill",
    "title": "/deploy uploads install.sh and checks both served URLs",
    "check": "f=.claude/skills/deploy/SKILL.md && grep -qF 'cp install.sh \"$D/install.sh\"' \"$f\" && grep -qF '\"$D/install.sh\"' \"$f\" && grep -qF 'releases/latest/download/install.sh' \"$f\" && grep -qF 'get.toolu.sh/pkg/toolu/install' \"$f\" && test \"$(grep -cE '^[0-9]+\\. \\*\\*' \"$f\")\" -eq 11",
    "ac_refs": ["AC-3"],
    "paths": [".claude/skills/deploy/SKILL.md"],
    "input": "The skill text.",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Release docs list install.sh and the get.toolu.sh redirect",
    "check": "grep -qF 'releases/latest/download/install.sh' docs/releases/native.md && ! grep -qF 'pin in `CodaSignal/toolu.sh` must move' docs/releases/native.md && cargo xtask check-markdown-cli",
    "ac_refs": ["AC-4"],
    "paths": ["docs/releases/native.md"],
    "input": "The final docs.",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Gate every step except tests and coverage, plus the xtask unit tests for this module",
    "check": "env -u npm_config_store_dir TMPDIR=/private/tmp TOOLU_RESOURCE_HOME=$(mktemp -d) cargo xtask gate --only gate-change --only fmt --only clippy --only guardrails --only layers --only reach --only deny --only machete --only unused-pub --only hooks --only workflows --only docs-cli --only cli-compat --only jscpd --only rust-quality --only docs --only context-budget --only final-removal --only gate-coverage --only packaging --only ci-paths --only bench --only pack && cargo test -p xtask workflow_install::tests && cargo test -p xtask check_workflows",
    "ac_refs": ["AC-6"],
    "depends_on": ["contract-check", "workflow", "skill", "docs"],
    "paths": [".github", "crates/xtask", ".claude/skills/deploy", "docs"],
    "input": "The final committed tree.",
    "model": "inherit"
  }
]
```

## Critical files

`crates/xtask/src/workflow_install.rs`, `crates/xtask/src/tests/workflow_install_test.rs`, `.github/workflows/release-native.yml`, `.claude/skills/deploy/SKILL.md`, `docs/releases/native.md`.

## Verification

- The empty-root unit test shows the new message, and the real workflows pass `check-workflows`.
- The skill's step 10 compares the served installers on the live run.
- AC-5 is deferred to the v8.4.1 `/deploy`, after the `CodaSignal/toolu.sh` worker deploy.
- Delivery:
  1. Commit.
  2. Run `toolu ledger run … --verify`.
  3. Run the review.
  4. Run `toolu ledger verdict status` until it reports `overall: ready`.
  5. Push and open the PR.

## Plan review (2026-10-26)

- gate: 🔵 consider: the `tests` and `coverage` steps fail on this host, as recorded in the manual-deploy plan. The touched xtask module's tests run explicitly instead.
- Every AC maps to a step, except AC-5, which is deferred to the live run. No blockers.
