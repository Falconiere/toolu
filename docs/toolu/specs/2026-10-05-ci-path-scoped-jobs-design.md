# CI path-scoped jobs with always-reporting required checks — Design

**Date:** 2026-10-05   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** #458: run only the CI jobs a change needs; required checks never stay Pending

## Problem

`tests.yml` runs the whole `bun run test` gate and the 60-minute OpenCode acceptance on
Linux and macOS for any change outside the release-only list. That includes a docs-only
change. `tests.yml` and `toolu-review.yml` skip through workflow-level `paths-ignore`.
GitHub leaves a required check from a path-skipped workflow **Pending**, so a version-bump
PR can merge only with `--admin`. Today's list also misses `tools/toolu-cli/npm/package.json`,
which release-please bumps, so release PR #390 never matched it and ran everything anyway.
The Rust epic (#402) adds more jobs (#407, #456), and each of them would multiply that cost.

## Non-Goals

1. No `rust` group and no Rust legs: #407 adds the group and #456 the jobs and `cargo xtask check-workflows`.
2. No change to which contexts branch protection requires (`typescript`, `review`, `gitleaks`, `Opengrep OSS`).
3. No change to `release-please.yml` or `npm-publish.yml`. Neither reports a required check or uses path filters.
4. No third-party paths-filter action. The decision is a script in `tooling/`.
5. No change to what `bun run test` (`test:ts`) runs locally.

## Architecture

The groups live as data in `.github/ci-paths.json`. Each gating workflow runs a `changes`
job first, which executes `bun run tooling/src/ci-changes.ts`. The script computes the
changed files with `git diff --name-only --no-renames` and writes one `true`/`false` output
per group, plus `changed`, to `$GITHUB_OUTPUT`. The ranges are:

- `pull_request`: `base.sha...head.sha`;
- `push`: `before..after`;
- `workflow_dispatch`: every group on.

Gated jobs carry a job-level `if: needs.changes.outputs.<group> == 'true'`. A job skipped by
its `if` reports Success.

The required aggregate `typescript` keeps `if: ${{ always() }}`. It runs
`bun run tooling/src/ci-aggregate.ts tests.yml` with `NEEDS: ${{ toJSON(needs) }}`. The script
reads the job→group map from the data file and applies the pass/fail rules below. Both
workflows lose `paths-ignore`.

The `review` job in `toolu-review.yml` is gated on the synthetic `changed` group, which is
true when any non-release-only file changed. Its `if` is
`!cancelled() && (needs.changes.result != 'success' || needs.changes.outputs.changed == 'true')`:
if `changes` crashed, review runs instead of reporting a skipped Success. `review` is itself
the required context, so it has no separate aggregate.

`bun run check:ci-paths` (`tooling/src/check-ci-paths.ts`, added to `test:ts`) enforces the
invariants statically over the real workflow YAML (`Bun.YAML.parse`) and `git ls-files`.
Shared logic sits in `tooling/src/ci-paths/` (schema, glob matching, classification,
aggregate rules, workflow checks) and reuses `zod` and `Bun.Glob`.

**Decisive trade-off: the `opencode` group is derived from the acceptance's import closure,
not only the issue's globs** (Jev choice `closure`, 0.94). The issue's
`tooling/src/opencode-*/**` misses the entry `tooling/src/opencode-acceptance.ts`, the
`opencode-*-smoke.ts` scripts, `tooling/src/env.ts`, `tooling/src/npm-pack.ts`,
`tools/toolu-conformance/**`, and the two docs whose bash blocks the acceptance runs verbatim
(`docs/opencode.md`, `docs/opencode-migration.md`). A test bundles the acceptance entries and
`*.live.test.ts` files with `Bun.build({ metafile: true })` and fails when a first-party input
lies outside the group. The group therefore cannot silently drift.

**Release-only is decided by path and content** (Jev choice `by_content`, 0.67). A
release-only file counts as release-only only when it is `CHANGELOG.md`, or when every
added/removed line of its diff is a JSON `"version"`, `"."` or `"@toolu/core"` pair with a
semver value. Those are exactly the fields that `release-please-config.json` writes. A
human edit of the root `package.json` scripts then runs the `ts` group instead of being
skipped, as it is today. If the rule is wrong it only runs more.

**Docs job.** A new script `test:docs` runs the checks that read `docs/**` or root Markdown:
`guardrails`, `test:portable-core` (`check-portable-core-doc`, `check:opencode-host`,
`check:opencode-docs` and their tests), `test:gate-coverage`, `test:final-removal`,
`check:ci-paths`, and `bun test` over `workspace-skeleton`, `install-prompts` and
`conventions-guardrails`. It took about 10 s locally.

## Interfaces / Schema

`.github/ci-paths.json`:

```json
{
  "groups": {
    "ts": ["packages/**", "plugins/**", "tools/**", "tooling/**", "benchmarks/**", "package.json", "bun.lock",
           "tsconfig*.json", ".oxlintrc.json", ".oxfmtrc.json", "knip.json", ".jscpd.json",
           "guardrails.workspace.json", "release-please-config.json", "LICENSE", ".claude-plugin/**", ".agents/**"],
    "opencode": ["tools/toolu-opencode/**", "tools/toolu-conformance/**", "plugins/**", "packages/toolu-core/**",
                 "tooling/src/opencode-*/**", "tooling/src/opencode-*.ts", "tooling/src/env.ts", "tooling/src/npm-pack.ts",
                 "docs/opencode.md", "docs/opencode-migration.md", "package.json", "bun.lock"],
    "docs": ["docs/**", "**/*.md"]
  },
  "runEverything": [".github/**", "bun.lock"],
  "releaseOnly": {
    "paths": ["CHANGELOG.md", ".release-please-manifest.json", "package.json", "packages/*/package.json",
              "tools/*/package.json", "tools/*/npm/package.json",
              "plugins/*/.claude-plugin/plugin.json", "plugins/*/.codex-plugin/plugin.json"],
    "versionKeys": ["version", ".", "@toolu/core"]
  },
  "workflows": {
    "tests.yml": { "aggregate": "typescript", "jobs": { "gate": "ts", "opencode": "opencode", "docs": "docs" } },
    "toolu-review.yml": { "aggregate": null, "required": ["review"], "jobs": { "review": "changed" } }
  }
}
```

In `tests.yml`, the `typescript` aggregate is itself the required context. A workflow lists
in `required` any other job names that are required contexts.

- `ci-changes.ts`: reads `GITHUB_EVENT_NAME`, `GITHUB_EVENT_PATH` (JSON with
  `pull_request.base.sha`/`head.sha`, or `before`/`after`) and `GITHUB_OUTPUT`. It appends
  `ts=…`, `opencode=…`, `docs=…` and `changed=…`, and prints the reason and the files per
  group to stdout. It exits 0 on every fail-open path and non-zero only when it cannot read
  the data file or write the output.
- `ci-aggregate.ts <workflow-file>`: reads `NEEDS` (GitHub's `toJSON(needs)`:
  `{ job: { result, outputs } }`). It exits 0 on pass and 1 on fail, and prints one line per
  job with its group, result and verdict.
- `check-ci-paths.ts [--root <dir>]`: exits 1 with one line per violation, 0 when clean.
  `--root` points it at a fixture tree.
- `package.json` scripts: `check:ci-paths`, `test:docs`. `test:ts` gains `check:ci-paths`.

Classification, for each changed path:

1. The file is release-only by path and content → it contributes nothing.
2. It matches `runEverything` → every group is on.
3. It matches one or more groups → those groups are on.
4. It matches nothing → every group is on.

`changed` is true when any file is not release-only.

## Failure modes and edge cases

- **`workflow_dispatch`, a push whose `before` is all zeros, a missing event field, an empty diff, or a `git diff` error** (for example an unknown SHA): every group and `changed` turn on, and the reason is logged. An empty diff is added to the issue's list: no real PR or push to `main` produces one, so it signals a wrong range, and running everything is the safe reading.
- **A file matches no group** (a new top-level directory): every group turns on.
- **`.github/**`, including `ci-paths.json`, or `bun.lock` changes:** every group turns on. These cover the issue's "root lockfile or toolchain file" trigger. The only root lockfile is `bun.lock`. The Bun toolchain pin (`bun-version`) lives in the workflow files, under `.github/**`. #407 adds `rust-toolchain.toml` and `Cargo.lock` to `runEverything`.
- **A release-only path with a non-version line** (a script edit in `package.json`): classified by its groups. Root `package.json` → `ts` and `opencode`.
- **A release-only file that is added or deleted:** every line counts, so it is not release-only and falls through to the groups.
- **Rename:** `--no-renames` lists the old and the new path, and both are classified.
- **`changes` fails or is cancelled:** `typescript` fails and names `changes`; it never passes on missing outputs. `review` runs (fail open).
- **A needed job ends `failure` or `cancelled`** (GitHub reports a job timeout as `failure`): `typescript` fails and names the job. A job `skipped` while its group is on also fails `typescript`. A job `skipped` with its group off passes; `success` passes either way.
- **`NEEDS` lacks a gated job, or lists an unmapped one:** `typescript` fails (a needs/data mismatch).
- **Check failures:**
  - a workflow in the data file, or one that produces a required context, has `paths` or `paths-ignore` under `push` or `pull_request`;
  - an aggregate's `needs` ≠ `changes` + its gated jobs;
  - a gated job's `if` does not reference `needs.changes.outputs.<its group>`;
  - a job references `needs.changes.outputs` without a data mapping;
  - a mapped group is not defined;
  - a glob matches no tracked file;
  - a workflow with a `changes`-gated job is not listed in the data file.

## Acceptance criteria

- **AC-1:** Given a git diff that changes only `docs/statusline/README.md`, `ci-changes` outputs `docs=true ts=false opencode=false changed=true`. `ci-aggregate tests.yml` passes on `{changes: success, docs: success, gate: skipped, opencode: skipped}`. In the workflow, `docs` runs `test:docs` and `review` is gated only on `changed`.
- **AC-2:** Given a diff shaped like release PR #390 (version-line bumps in every release-only file, including `tools/toolu-cli/npm/package.json`, plus a CHANGELOG entry), `ci-changes` outputs every group and `changed` as `false`. The aggregate passes with every gated job skipped, and neither workflow has `paths`/`paths-ignore`. Given a diff that changes a `scripts` line in root `package.json`, `ts=true`.
- **AC-3:** Given a diff that changes `tools/toolu-opencode/src/index.ts`, `ts=true opencode=true`, and the workflow gates `gate` on `ts` and the two-OS `opencode` matrix on `opencode`.
- **AC-4:** Given a diff that adds `newdir/file.txt`, or that edits `.github/workflows/tests.yml`, `.github/ci-paths.json` or `bun.lock`, every group is `true`.
- **AC-5:** `ci-aggregate` fails and names the job when:
  - `changes` is `failure`;
  - a needed job is `failure` or `cancelled`;
  - `gate` is `skipped` while `ts=true`.

  `check-ci-paths` fails on a fixture where `ci-paths.json` drops the `docs` job mapping.
- **AC-6:** On a `push` event, `before..after` is classified with the same groups. An all-zero `before`, an unknown SHA and `workflow_dispatch` each turn every group on.
- **AC-7:** `check-ci-paths` passes on the repository. It fails on fixtures that put `paths-ignore` on `tests.yml`, mismatch an aggregate's `needs`, carry a glob matching no tracked file, or leave a gated job without a group.
- **AC-8:** Every first-party input in the `Bun.build` import closure of `tooling/src/opencode-*.ts` and `tools/toolu-opencode/**/*.live.test.ts` matches the `opencode` group.
- **AC-9:** The CI table in AGENTS.md lists `changes`, `gate`, `opencode`, `docs` and `typescript` (with their groups and the aggregate) and the `review` gating.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Temp git repo seeded with real tracked paths; a commit editing `docs/statusline/README.md`; real `ci-paths.json`; real `GITHUB_OUTPUT` file | Outputs as stated; aggregate exit 0 | `README.md` at the root also maps to docs only | `bun test tooling/src/__tests__/ci-changes.test.ts tooling/src/__tests__/ci-aggregate.test.ts` |
| AC-2 | Temp repo with copies of the real release-only files, bumped exactly like #390 | All false; aggregate exit 0 | A `scripts` line in `package.json` → `ts=true` | same, plus `ci-workflows.test.ts` |
| AC-3 | Temp repo commit editing `tools/toolu-opencode/src/index.ts` | `ts`, `opencode` true; `docs` false | — | `ci-changes.test.ts`, `ci-workflows.test.ts` |
| AC-4 | Commits adding `newdir/file.txt`; editing `.github/workflows/tests.yml`, `.github/ci-paths.json`, `bun.lock` | All true | — | `ci-changes.test.ts` |
| AC-5 | Real `toJSON(needs)` shapes; fixture copy of the repo's `.github` minus the `docs` mapping | Exit 1 naming the job; check exit 1 | `skipped` with group off passes | `ci-aggregate.test.ts`, `check-ci-paths.test.ts` |
| AC-6 | Push event JSON with real SHAs, `0000…` before, a bogus SHA; dispatch event | Same groups; fail open | — | `ci-changes.test.ts` |
| AC-7 | The repository; fixture trees copied from the real `.github`, mutated one way each | Exit 0 on the repo; exit 1 with the named violation on each fixture | — | `bun run check:ci-paths`, `check-ci-paths.test.ts` |
| AC-8 | The real source tree | No uncovered inputs | — | `ci-paths-closure.test.ts` |
| AC-9 | AGENTS.md | Table names jobs, groups, aggregate | — | `workspace-skeleton.test.ts` assertion |

On GitHub, this PR edits `.github/**`, so its own run must start every job through the
fail-open path, and `typescript` and `review` must report. Whether a docs-only PR stays
green is proven by the script tests above. The first such PR after merge confirms it live. The same goes for the next release-please PR: whether it merges without `--admin` can only be observed then.

## Documentation impact

- AGENTS.md CI table: jobs, groups, the aggregate, review gating, no workflow-level filters. The docs-sync surface (CLAUDE.md imports AGENTS.md).
- `docs/testing.md`: a short note on `test:docs` and `check:ci-paths`.
- Comments in `tests.yml` and `toolu-review.yml` replace the `paths-ignore` rationale.
- `release-please.yml`'s token comment mentions the skip; update it to say the jobs skip through `if`.

## Open Questions

None blocking. Every decision above is recorded with its reason; the orchestrator owns any later change.

## Spec review

- Failure modes: 🟡 should-fix: the empty-diff fail-open had no reason. Fixed: reason stated.
- Failure modes: 🟡 should-fix: the issue's toolchain-file trigger was not mapped. Fixed: `bun.lock` and the workflow-held Bun pin are covered; #407 owns the Rust files.
- Acceptance evidence: 🔵 consider: "merges without `--admin`" is only observable live. Fixed: stated as post-merge evidence.

Coverage, checked bullet by bullet against #458 (Jev coverage 0.63 and contradiction 0.44 were inconclusive, so checked by hand):
- Scope: data file, `changes`, fail open, gating, docs job, aggregates, no workflow filters, review, check and docs map to Architecture, AC-1…AC-9.
- Acceptance: the seven checkboxes map to AC-1, AC-2, AC-3, AC-4, AC-5, AC-6 and AC-7.
- Scenarios: the `changes` crash maps to AC-5. The Rust scenario is a non-goal (#407).
- Deviations: the wider `opencode` group, the content-checked release-only rule, the added `tools/*/npm/package.json` and the empty-diff fail-open are additions, each with a stated reason.

**Status:** Approved
