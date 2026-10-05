# OpenCode install docs and verified migration guide — Design

**Date:** 2026-10-04   **Status:** Approved   **Author:** Falconiere Barbosa (epic worker, Claude)   **Topic:** #363 (OP-29, epic #334): replace OpenCode install docs, publish a migration guide, and tie every support claim to CI evidence

## Problem

The OpenCode adapter now follows the documented plugin API, and CI runs a required real-host acceptance on it (#362). The user-facing docs still mix that path with claims from the V2 era:

- `tools/toolu-opencode/README.md` ships inside the npm tarball. It still says the adapter wires `permission.evaluate`, requires OpenCode `v2.0.12`, and has no post-tool checks.
- The root `README.md` says the adapter "wires `permission.evaluate`" and that coverage is limited to #212.
- `docs/opencode.md` is headed by the closed issues #207 and #203 and says "#363 replaces this install guide". It warns against a "legacy `opencode-ai@1.18.31` (V1) line", which is the same line as the pin. Its rollback and disable steps cover only the clone shim, and a note on migrating from manual wiring is buried in step 5.
- No document tells a V2 user how to move to the documented line, which files toolu owns, or how to roll back.
- No user-facing page states the support status of each of the 16 plugins on OpenCode. The capability matrix lives only in the host contract, and nothing ties it to the CI acceptance checks.
- Nothing proves that the documented steps work as written. The `cli.*` scenarios hand-code their own steps.

## Non-Goals

1. No change to adapter, CLI, bootstrap or gate behavior. If a documented step fails, the docs are fixed, unless a real defect is found; a real defect is fixed narrowly and recorded.
2. No retained V2 host support. Releases from 7.8.0 load only on `opencode-ai@1.18.34`, and the V2 entry shape is the `control.v2-entry` regression control. The guide says so explicitly.
3. The historical `smoke:clean-install` V2 lane and the internal `createPermissionEvaluateHandler` export are not removed (out of scope). Their docs label them as internal and historical, not as an install route.
4. Windows stays N/A. No new platforms.
5. No Claude Code or Codex doc or behavior changes beyond removing shared stale wording.

## Architecture

Three pieces. Each one turns a documentation claim into something a check enforces.

1. **Executed documentation blocks (AC1, migration).**
   - `docs/opencode.md` gets a `## Quick start` section. Its single fenced `bash` block sits between `<!-- opencode-doc:quickstart:start -->` and `<!-- opencode-doc:quickstart:end -->`. A `manage` block, with the same markers, holds the update, disable-a-plugin and remove-toolu commands.
   - The new `docs/opencode-migration.md` has a `migrate` block and a `rollback` block with the same markers.
   - A new live scenario module, `tooling/src/opencode-host/scenarios-docs.ts`, extracts each block and runs it **verbatim** with `bash -eu` in an isolated profile. The isolated profile is the existing `entrySession`: a temp HOME/XDG, a git project and the scripted loopback provider.
   - Only two commands are redirected, through a `PATH` shim directory:
     - `npx @toolu/plugins …` runs the checkout's CLI bundle under Node, built as `scenarios-cli.ts` already does;
     - `opencode` runs the pinned binary.
   - `TOOLU_OPENCODE_PACKAGE` points the CLI at the packed tarball, because the release under test is not yet on npm. Every other command (`printf`, `grep`, `tar`, `npm pkg`, `rm`) is the reader's own.
   - The scripted provider's `*` script answers the quick start's untagged `opencode run` prompt with a `write` of `.env`.
   - Reuse:
     - `entrySession`, `protectedWrite`, `enforced`, `GATED_FILES` and `npmSpec` from `scenarios-entry.ts`;
     - `skills()` and `catalogIds()` from `install-host.ts`;
     - the CLI build, exported from `scenarios-cli.ts` instead of duplicated (jscpd threshold 0).
   - The scenarios register as family `docs` in the acceptance registry (`families.ts`) and in `smoke:opencode-entry`. CI therefore runs them on Linux and macOS, and a doc edit that breaks a step fails the required check.
2. **Generated per-plugin support table (AC2).**
   - `docs/opencode.md` gets a `## Plugin support` section with a block between `<!-- opencode-support:start -->` and `<!-- opencode-support:end -->`.
   - The block is rendered from `contract/capability-matrix.json` and from the acceptance registry (`acceptanceChecks(committed)` plus `coverage()` in `checks.ts`). The rendering treats every check as passing: the table lists the dedicated actual-host checks that CI requires to pass for each plugin.
   - Status per plugin:
     - `Supported` when every used axis is `supported` and the row has no notes;
     - `Supported with limitations` when any used axis is `partial` or `unsupported` with an alternative, or the row has a note;
     - `Blocked` when any cell is a release blocker (none today).
   - Limitations are rendered as bullets, verbatim from the matrix's `alternative` and note text.
   - A new hermetic checker, `tooling/src/check-opencode-docs.ts` (`bun run check:opencode-docs`, wired into `test:portable-core`), compares the block with the rendering. `--write` rewrites it. The check mirrors `check:opencode-host` and its `doc-blocks.ts` helpers, which are reused (`readBlock`/`writeBlock` generalized to take marker names).
3. **Stale V2 claim guard (AC3).**
   - The same checker scans the documented target path for V2-only claims: `README.md`, `docs/opencode.md`, `docs/cli.md`, `docs/plugins/index.md` and `tools/toolu-opencode/README.md`.
   - Forbidden: `opencode plugin add|check|update|remove`, `@opencode/plugin`, `@opencode/cli`, `2.0.12`, `opencode.ai/v2/`, `permission.evaluate`, `1.18.31`, and the manual surface copy `generated/skills`. Every hit is named with its file and line.
   - `docs/opencode-migration.md` and the historical `docs/conformance-report.md` are exempt: they must name V2 artifacts.

**Decisive trade-off.** Running the doc blocks costs two to three extra host sessions per acceptance run, about 1–2 minutes. In exchange, the docs cannot claim a step that CI does not perform (Jev: choice A, confidence 1.0, over citing the hand-coded scenarios). The generated table and the separate migration doc were chosen the same way (Jev: C and E, confidence 1.0).

**Migration facts the guide relies on (verified in code):**

- The V2 `opencode plugin add` wrote the package into `$XDG_CONFIG_HOME/opencode/opencode.json` (`tooling/src/clean-install-smoke.ts`). The pinned host reads that same file.
- `toolu update --host opencode` rewrites every `@toolu/opencode` entry, unpinned ones included, to the CLI's release and keeps tuple options and comments (`tools/toolu-cli/src/opencode/update.ts`).
- `plugins.json` and `toolu.config.json` keep the same schema and paths.
- A V2 clone shim `.opencode/plugins/toolu.ts` that is left in place loads a second instance, which logs `toolu: duplicate load skipped`.
- The data root `.opencode/toolu/state/` is toolu-owned and rebuilt at each start.
- The last V2-targeted release is 7.7.2. From 7.8.0 on (#366), the package loads only on the documented line.

## Interfaces / Schema

- Doc markers: `<!-- opencode-doc:<name>:start -->` and `<!-- opencode-doc:<name>:end -->`, with exactly one fenced `bash` block between them. The names are `quickstart` and `manage` (in `docs/opencode.md`) and `migrate` and `rollback` (in `docs/opencode-migration.md`).
- `tooling/src/opencode-host/doc-commands.ts`:
  - `docBlock(text: string, name: string): string`, which throws `ContractError` when the markers are missing, duplicated, or do not wrap exactly one fenced `bash` block;
  - `runDocBlock(ctx: EntryContext, s: ProbeSession, script: string): Promise<{ exitCode: number; stdout: string; stderr: string }>`.
- `scenarios-docs.ts` exports `DOCS_SCENARIOS: EntryScenario[]` with ids `docs.quickstart` and `docs.migration`. The plugins are `toolu` and `context7` for the quick start, which runs `quickstart` and then `manage` in one profile, and `toolu` for the migration; the service is `none`.
- `scenarios-cli.ts` exports `cliBundle(ctx)`, the existing function, now shared.
- Support block markers: `<!-- opencode-support:start -->` and `<!-- opencode-support:end -->`. The renderer is `renderSupport(matrix: Matrix, coverage: Record<string, string[]>): string` in `tooling/src/opencode-acceptance/support-doc.ts`.
- Root script: `"check:opencode-docs": "bun run tooling/src/check-opencode-docs.ts"`. It is appended to `test:portable-core`. `--write` regenerates the support block.
- Env overrides for tests: `TOOLU_OPENCODE_DOC`, `TOOLU_OPENCODE_MIGRATION_DOC` and `TOOLU_DOCS_ROOT`, the root that the guard's relative file list resolves against.

## Failure modes and edge cases

- **Doc markers missing or broken.** The markers are absent, duplicated, or wrap no `bash` fence or more than one. `docBlock` throws `ContractError` naming the doc and the block. The scenario fails, never skips.
- **A documented command fails.** `bash -eu` exits nonzero. The scenario fails with the exit code and the stderr tail. If `opencode run` exits nonzero after a denied tool call, the doc block must show what a reader sees. This is decided empirically in execution and recorded in the ledger.
- **`npx` called with another package.** The shim exits 97 with the message `doc shim: only @toolu/plugins is redirected`. A doc can therefore never call an unpinned network package unnoticed.
- **Quick start passes trivially.** This happens if the model never attempted the write. The scenario requires the scripted provider's recorded requests to include a tool result carrying toolu's protected-file denial, and requires `.env` to keep its original bytes.
- **Migration with toolu configured in both scopes.** The block is written for the V2 default (global entry). The guide states `--scope` for the both-scopes case, which the CLI rejects with exit 2. Not executed.
- **Rollback.** The `rollback` block restores the tarred files. The scenario checks that the global config, the selection, `toolu.config.json`, `.opencode/package.json` and the shim are byte-equal to the pre-migration seed.
- **Support table drift.**
  - Changing the matrix or the acceptance registry without `--write` exits 1 with `support block is stale; run bun run check:opencode-docs --write`.
  - A plugin with no dedicated check renders `none` and fails the checker. The table can never claim support for a plugin that CI does not cover.
- **Stale claim.** Exit 1 with `file:line: <pattern>` per hit.
- **Missing doc file.** `ContractError` naming the path.

## Acceptance criteria

- **AC-1:** In a clean isolated profile, running the `docs/opencode.md` quick-start block verbatim leaves the global config with exactly one `@toolu/opencode` entry and the selection `toolu` plus `context7`. `opencode debug skill` lists `context7-context7`, and the block's `opencode run` write to `.env` is denied before execution: `.env` keeps its bytes and the model receives toolu's protected-file reason.
- **AC-7:** Continuing in AC-1's profile, running the `manage` block verbatim:
  - leaves the `@toolu/opencode` entry at the tested spec (update reports it current);
  - removes `context7-context7` from `opencode debug skill` once `context7` is disabled;
  - after toolu is removed, leaves no `@toolu/opencode` entry and no toolu skill, while keeping the selection file.
- **AC-2:** In a profile seeded with V2-era state, running the `docs/opencode-migration.md` `migrate` block verbatim gives these results:
  - toolu loads once on the pinned host (one `toolu: ready`, no duplicate);
  - the user's selection and `toolu.config.json` stay byte-equal;
  - the V2 `@opencode/plugin` dependency and the shim are gone;
  - user comments and the other plugin entry are kept;
  - a protected write is denied.

  The V2-era state is a global commented `opencode.jsonc` holding an unpinned `@toolu/opencode` entry and another plugin, a `.opencode/plugins/toolu.ts` shim, a `.opencode/package.json` with an `@opencode/plugin: 2.0.12` dependency, a selection and a `toolu.config.json`.
- **AC-3:** Running the `rollback` block after AC-2 restores every seeded file byte for byte.
- **AC-4:** `docs/opencode.md` § Plugin support lists all 16 catalog plugins.
  - Each row shows a status derived from `capability-matrix.json`, its host-specific limitations, and the dedicated actual-host checks that `bun run test:opencode` requires for it.
  - `bun run check:opencode-docs` exits 0 on the committed docs, and exits 1 naming the block when a row is edited or the matrix changes.
- **AC-5:** `bun run check:opencode-docs` exits 0 on the target-path docs. Inserting any forbidden V2 pattern into one of them makes it exit 1 with `file:line`. The migration guide is exempt.
- **AC-6:** The user-facing docs agree with the tested behavior: the README, `docs/plugins/index.md` prompts, `docs/cli.md`, `tools/toolu-opencode/README.md`, `docs/portable-core.md`, `docs/conformance-report.md`, the npm-publish workflow comment and the regenerated `tools/toolu-opencode/generated/` copies. `bun run test` passes, including install-prompt parity, `check:opencode-surface`, the context budget and the portable-core doc check.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | `docs/opencode.md` quick-start block, packed tarball, pinned `opencode-ai@1.18.34` | Entry count 1, selection `["toolu","context7"]`, skill listed, `.env` unchanged, denial in a recorded tool result | `npx` shim refuses other packages; the model must have attempted the write | `bun run smoke:opencode-entry docs.quickstart`; `bun run test:opencode` (CI, Linux and macOS) |
| AC-7 | `manage` block after AC-1 | Spec unchanged, `context7-context7` gone, then no toolu entry or skill, selection kept | Disabling a plugin with no dependents; removing the package keeps the selection | `bun run smoke:opencode-entry docs.quickstart` |
| AC-2 | Migration block plus the seeded V2-era profile | Ready 1, duplicate 0, write denied, files kept, V2 dependency removed | Unpinned V2 spec; commented JSONC | `bun run smoke:opencode-entry docs.migration` |
| AC-3 | Rollback block after migration | Seeded files byte-equal | Files the host created at startup | same scenario |
| AC-4 | Committed matrix plus acceptance registry | Block equal to the rendering; 16 rows | Edited row, a plugin with no dedicated check | `bun run check:opencode-docs`; `bun test tooling/src/__tests__/check-opencode-docs.test.ts` (sandbox copies of the real docs) |
| AC-5 | Real target-path docs | Exit 0; injected pattern gives exit 1 with `file:line` | Migration doc exempt | same test file |
| AC-6 | Repository docs | All gates green | Generated mirror drift | `bun run test` |

## Documentation impact

- `docs/opencode.md` is rewritten: a quick start, the per-plugin support table, the CLI-centred update/rollback/remove steps, and a contributor clone path kept separate.
- `docs/opencode-migration.md` is new.
- Also updated: `README.md` (OpenCode section), `docs/plugins/index.md` (mirrored prompt), `tools/toolu-opencode/README.md`, `docs/portable-core.md` (retained V2-shaped exports labeled internal), `docs/conformance-report.md` (the #279 V2 paragraph labeled historical; new `docs` checks listed), `.github/workflows/npm-publish.yml` (comment), `docs/cli.md` (link to the migration doc) and `AGENTS.md` (key-files row for `check:opencode-docs`).
- `tools/toolu-opencode/generated/` is regenerated.

## Open Questions

- Does `opencode run` exit nonzero when its only tool call is denied? This is non-blocking. Execution observes it; if nonzero, the quick-start block shows the documented outcome (for example `opencode run … || true`, with a comment) rather than hiding it. Owner: this worker.

## Spec review

- Acceptance criteria: 🟡 should-fix: the issue scope covers update and remove instructions, but no AC executed them (Jev scope-coverage noul 0.67). Fixed: a `manage` block and AC-7, run in `docs.quickstart` (re-check noul 0.83).
- Acceptance evidence: 🔵 consider: `opencode debug skill` is a host debug subcommand rather than a page in the plugin docs. Kept: it is the pinned host's own CLI, and the generated skill also appears in the system prompt a reader sees.
- Problem / Non-Goals / Architecture / Failure modes / Docs impact / Open questions: no findings. The open question is owned, non-blocking, and resolved empirically in execution. Jev coverage of issue criteria I1/I2/I3: 0.85/0.82/0.85.

**Status:** Approved
