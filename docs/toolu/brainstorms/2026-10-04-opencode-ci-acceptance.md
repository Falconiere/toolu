# OpenCode CI acceptance (#362, OP-28) — brainstorm

**Path:** Full (CI, public check names, external cost, cross-cutting test layout).

## Capsule

- **Outcome:** one command, `bun run test:opencode`, runs every live OpenCode check on the pinned host, records what each check proved and how, and fails on any failure, skip, coverage gap, undetected regression control or exceeded budget. CI runs it on Linux and macOS, and the required `typescript` status fails when it fails.
- **Material defaults/non-goal:** reuse the existing scenarios, harness and smoke scripts; do not rewrite them. Remove the version-only `live-opencode` suite and the handwritten `permission-evaluate` suite. Live calls to real external services (context7.com, TypeSafe) stay opt-in and are reported apart from host acceptance. No repository-settings change. Windows stays N/A.
- **Repository evidence:** about 100 live scenarios already exist across `tooling/src/opencode-*-smoke.ts`, `probe:opencode-host`, `probe:opencode-surface` and seven `*.live.test.ts` files, and together they reach all 16 plugins. None runs in CI. The host contract doc assigns macOS acceptance to this issue. On macOS arm64 the probe family took 93 s and the 34 entry scenarios took 208 s.
- **Risk:** CI time (two extra jobs of about 10–20 min, run in parallel with the gate), macOS-only host differences, and timing budgets that turn flaky on shared runners. The runner records generous ceilings and the measured values.
- **Handoff:** spec.

## Axes

### Interface: one acceptance command

- **Default:** `tooling/src/opencode-acceptance.ts`, run as `bun run test:opencode [--report <file>] [--only <check>…]`. It imports the existing scenario arrays in-process, runs the `*.live.test.ts` files as `bun test` subprocesses with `TOOLU_LIVE_OPENCODE=1` and a JUnit report, and writes one JSON report plus a Markdown summary (also to `$GITHUB_STEP_SUMMARY`).
- **Evidence:** every smoke script already exports its scenario array and shares `ScenarioContext`, `resolveHostBinary` and `hostCacheDir`.
- **Rejected:** spawning each `smoke:*` script and scraping stdout. That gives no per-check timing or typed evidence, and repeats host resolution and packing.

### Failure behavior: nothing skips

- A missing host, missing tool (`ast-grep`, `agent-browser`, Chromium, `npm`, `git`) or failed install is a failure. Scenarios already throw `ContractError` for these; a preflight records tool versions and fails first, with one clear message.
- A JUnit `skipped` host test is a failure. Only the declared external-service tests may be skipped, and only when their key is absent; they are reported as `not-configured`.
- A narrowed run (`--only`) is marked `complete: false` and never counts as acceptance.

### Regression controls (AC-1)

- **Chosen:** live mutation controls. Each run stages a copy of `@toolu/opencode`, applies one regression, runs the matching check on the pinned host, and passes only when that check fails. The regressions are: a V2 `Plugin.define`/`permission.hook("evaluate")` entry, an invalid skill name with `--`, the system-context hook removed, and `tool.execute.after` removed. Each edit asserts that it applied, so a source change cannot silently turn a control into a no-op. A hermetic test also applies the edits in `bun run test`.
- **Rejected:** hermetic predicate tests with fabricated observations, which never run the host, and a documented-only mapping.
- **Jev:** `live_mutations` 1.00.

### Budgets

- **Chosen:** a differential live check. The same scripted session runs on the real host with no plugin and with toolu selecting all 16 plugins. The scripted provider timestamps each request; the startup delta is the time to the first model request, and the per-tool overhead is the median delta of the gaps between requests. Ceilings live in `tools/toolu-opencode/contract/acceptance-budgets.json`; measured values go in the report.
- **Rejected:** per-call timing logs in the production plugin (noise and cost on every call), and suite wall time alone, which does not isolate toolu.
- **Jev:** `differential` 1.00.

### CI shape

- **Chosen:** an aggregator. The existing heavy job becomes `bun run test` (job id `gate`). A new `opencode` matrix job runs on `ubuntu-latest` and `macos-latest`. A small final job named `typescript` (`needs: [gate, opencode]`, `if: always()`) fails unless every needed job succeeded. The required context stays `typescript`, and it now covers acceptance, without serializing the jobs.
- **Rejected:** inline steps in the `typescript` job, which add 10–20 serial minutes and leave macOS unrequired; separate jobs that rely on an admin adding required contexts, which the worker cannot do. A plain `needs` without `if: always()` would let a skipped dependent count as passing.
- **Jev:** `aggregator` 0.96.

### Platforms

- **Chosen:** linux-x64 (`ubuntu-latest`) and darwin-arm64 (`macos-latest`). The repository is public, so macOS minutes are free. **Jev:** `linux_and_macos` 0.98.

### Coverage

- Each acceptance family declares the catalog plugins it proves and its evidence kind: actual host, scripted loopback model, and external service `none`, `fixture` or `live`.
- A hermetic test fails when any of the 16 catalog plugins lacks an actual-host check, or when the workflow stops running the acceptance on both platforms.

### New live checks

- **`concurrent.sessions`:** two `opencode run` processes in one project at once, plus a second project with a failing gate. Each run is ready and enforces, and only the failing project's commit is refused.
- **`surface.discovered-names`:** the skills the host discovers through toolu's config hook all satisfy the documented name rule and match the catalog.
- **`budget.overhead`:** the differential budget check above.

## Open risks

- macOS host behavior may differ from Linux. The baseline run on macOS arm64 is the first evidence, and CI gives the second.
- The budget ceilings are calibrated from local and CI measurements, with a wide margin so they catch pathological regressions rather than noise.
