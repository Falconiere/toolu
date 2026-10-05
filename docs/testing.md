# Testing with bun test

toolu uses `bun test` for its hook, tooling, and conformance suites ([epic #247](https://github.com/Falconiere/toolu/issues/247)). `bun run test` runs the TypeScript gate: conventions, real-subprocess unit and conformance tests, portable-core and gate-coverage checks, final-removal checks, bundle and launcher drift, package/workspace checks, context budget, deterministic benchmarks, and the shell-analysis latency measurement.

CI runs `bun run test` in the `bun run test` job (`gate`) when the change touches the `ts` path group of `.github/ci-paths.json`. Its shell-analysis cold-start budget is hard on macOS arm64 or with `TOOLU_LATENCY_ENFORCE=1`, and report-only on Linux. See [conformance-report.md](conformance-report.md) for measurements against the `v7.2.0` Bash baseline.

A docs-only change runs `bun run test:docs` instead: the checks and tests from `test:ts` that read `docs/**` or root Markdown. `bun run check:ci-paths` keeps the workflows and path groups consistent; see the CI section of AGENTS.md.

OpenCode acceptance (`bun run test:opencode`) runs in its own CI jobs, `opencode (ubuntu-latest)` and `opencode (macos-latest)` ([#362](https://github.com/Falconiere/toolu/issues/362)). It installs the pinned `opencode-ai` CLI and drives it in isolated profiles against a scripted loopback provider, through every live check, regression control and budget. They run when the change touches the `opencode` path group. The required `typescript` status fails when a needed job fails or is cancelled, and when a job is skipped although its group is on. Every `bun run test` also runs `bun run check:opencode-host`, which checks the committed evidence; see [opencode-host-contract.md](opencode-host-contract.md).

A test spawns the real thing: a hook bundle, `git`, `npm`, or a host CLI. It runs against real temp repositories and config roots. There are no mocks (AGENTS.md).

Shared helpers live in `@toolu/conformance/harness/*` (`tools/toolu-conformance/src/harness/`):

| Module | What it gives you |
|---|---|
| `sandbox` | `using sb = createSandbox({ git: true })`: a temp `project` (optionally a git repo with one commit), `home`, and `codexHome`. Helpers are `path`/`write`/`read`/`git`, plus `writeConfig(host, "project" \| "user", config)` for `toolu.config.json`. The tree is removed when the scope ends, even if the test throws. |
| `spawn` | `run(argv, { cwd, env, stdin, timeoutMs })` captures stdout, stderr, exit code, `durationMs` and `timedOut`. A timeout kills the whole process group. A missing binary resolves with exit 127. Host-session variables (`CLAUDE_*`, `CODEX_*`, `TOOLU_*`, `PLUGIN_ROOT`, …) never leak in, and an `env` value of `undefined` unsets a key. `runHook({ host, sandbox, pluginRoot, bundle \| argv, stdin })` spawns the real bundle as the host would. It sets `CLAUDE_PLUGIN_ROOT` / `PLUGIN_ROOT` / `CODEX_HOME` / `TOOLU_HOST_OVERRIDE` and runs with cwd at the project. |
| `hosts` | `readHostOutcome(host, event, result)` validates a hook's output against that host's contract and returns `{ effect: allow \| deny \| ask, reason?, context? }`. Claude and Codex use `hookSpecificOutput`, and Codex has no `ask`. Cursor answers with `permission` and an empty reply is invalid. `readOpencodeOutcome` reads the effect of an OpenCode permission event. A violation throws `HostOutputError`. |
| `fixtures` | Builders: `editFixture`, `writeFixture`, `patchFixture` (multi-file `apply_patch`), `bashFixture`, `mcpFixture`, `sessionFixture`, `promptFixture`, `postToolFixture`. `toStdin(host, fixture, { cwd })` renders the host's stdin: Codex receives edits as `apply_patch`, and Cursor gets `beforeShellExecution` / `beforeMCPExecution` / `preToolUse`. `toOpencodePermission` builds the in-process event. |
| `timing` | `measureLatency(once, { runs, warmup })` samples sequentially and reports p50/p95. `assertLatencyBudget(candidate, baseline, 5)` checks the cold-start delta measured in the same run when enforcement is enabled. |
| `entry-command` | The one place a test names a committed hook entry. `bundlePath(root, entry)` is `hooks/dist/<entry>.js` under `root`; `entryArgv(plugin, entry)` runs it with Bun; `launchedArgv({ plugin, event, entry })` runs it through its `hooks.json` launcher under `sh -c`; `resolveEntryCommand` is the resolver behind them. Each returns the selected Rust command instead under `TOOLU_IMPL` (below). `implementationTag(plugin, entry)` is a test-name suffix that names the Rust run. |
| `startup` | SessionStart startup hooks run the way a host runs them: `runStartupHook(pluginRoot, entry, sb, env)` executes the plugin's real `hooks.json` launcher under `sh -c`; `startupEnv(host, sb, pluginRoot)` / `startupRoot` give the Claude or Codex environment and config root. `publishedCliSuite(spec)` registers the shared cases for a plugin that publishes a Bun CLI at a stable config-root path (both hosts with and without credentials, stale link, user file, Bun off `PATH`, no Bun, missing bundle). |

A port test reads like this:

```ts
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { editFixture, toStdin } from "@toolu/conformance/harness/fixtures";
import { readHostOutcome } from "@toolu/conformance/harness/hosts";
import { bundlePath } from "@toolu/conformance/harness/entry-command";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { runHook } from "@toolu/conformance/harness/spawn";

const pluginRoot = resolve(import.meta.dir, "../../.."); // plugins/<name>
const bundle = bundlePath(pluginRoot, "<entry>");

test.concurrent("protected files deny an .env edit on codex", async () => {
  using sb = createSandbox({ git: true });
  sb.writeConfig("codex", "project", { version: 1, gates: { protectedFiles: { mode: "block" } } });
  const stdin = toStdin("codex", editFixture(sb.path(".env"), "a", "b"), { cwd: sb.project });
  const res = await runHook({ host: "codex", sandbox: sb, pluginRoot, bundle, stdin });
  expect(readHostOutcome("codex", "PreToolUse", res).effect).toBe("deny");
});
```

## Running the suites against a Rust port

The Rust rebuild ([epic #402](https://github.com/Falconiere/toolu/issues/402)) reuses these black-box tests unchanged ([#409](https://github.com/Falconiere/toolu/issues/409)). `TOOLU_IMPL` selects which hook entries run as the Rust binary:

| `TOOLU_IMPL` | Runs |
|---|---|
| unset or empty | every entry as its committed Bun bundle, exactly as before |
| `rust` | every entry as the Rust binary |
| `rust:<plugin>/<entry>,...` | only the listed entries as the Rust binary, e.g. `rust:toolu/pre-tools` |

Any other value, an empty item, a duplicate or a path separator inside a name is an error, never a silent fallback to Bun. The binary is `toolu` in `TOOLU_RUST_BIN_DIR`, or `target/release/toolu` at the repository root when that is unset. A selected entry runs `toolu hook <entry>` for the core `toolu` plugin and `toolu <plugin> hook <entry>` for the others, with the same stdin, cwd and environment the bundle gets. A selected binary that is missing, not a file or not executable fails the test at setup and names the selector and the expected path. The pre-tools case suites put `implementationTag` in their test names, so `TOOLU_IMPL=rust:toolu/pre-tools bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-golden.test.ts` reports a failing case as `… [rust:toolu/pre-tools]`.

Test files reach a bundle only through `entry-command`: `bundle-references.test.ts` fails on a test that spells the `hooks/dist/` path or calls `launcherCommand` itself, except for the listed files whose subject is the bundle or launcher text (packaging, drift, the launcher, and the OpenCode bootstrap, which spawns bundles in production code).

`test:conformance` runs four suites. `protected-files` and `spaces-cwd` dispatch `toolu/pre-tools` and take the Rust route when it is selected; `bootstrap-readiness` (OpenCode's missing-bundle diagnosis) and `surface-drift` (generated TypeScript) keep their checks in both modes.

`fixtures/rust-ported.json` lists the ported entries, `{ "entries": ["<plugin>/<entry>", ...] }`, and a port adds its entry there. `bun run test:rust-conformance` reads it: an empty list is a no-op, otherwise it runs `cargo build --release --locked --bin toolu` (`CARGO` overrides the cargo executable), then `test:unit` and `test:conformance` with `TOOLU_IMPL=rust:<entries>`. CI runs it in the `rust-conformance` job, which installs the Rust toolchain only when the list is not empty.

Epic-orchestrator's `report.test.ts` and `epic-watch.test.ts` test the TypeScript scripts' own behavior and stay as they are: #434 (the resident engine) and #435 (worker reports and actions) replace them with Rust black-box tests, so they have no `toolu epic` mapping here.

## Concurrency rule

bats ran files in parallel and tests within a file serially, because suites shared per-file state. `bun test` suites follow a stronger rule: **files in parallel, tests concurrent.**

- Write every test as a top-level `test.concurrent(...)`. Where a required tool may be absent, use `test.concurrent.skipIf(NO_TOOL)(...)`. CI installs every such tool.
- Each test owns its temp directories through `createSandbox`. Nothing is shared at module scope.
- Never call `process.chdir` and never write `process.env`. Pass `cwd` and `env` to `run` instead.
- Latency measurements are the exception: they time one process at a time, so a timing test stays a plain `test`.

The suites pass under `bun test --parallel --concurrent --timeout 60000`. Every `bun test` in the package scripts passes `--timeout 60000`, because real subprocesses under concurrency outrun bun's 5 s default on a loaded CI runner. Pass it too when you run several suite files by hand.

On macOS, put Homebrew OpenSSL 3 before `/usr/bin` on `PATH` when running the HTTPS fixture or the full TypeScript gate: `PATH=/opt/homebrew/bin:$PATH bun run test:ts`. Apple's `/usr/bin/openssl` is LibreSSL and its generated EC key fails to load in Bun 1.4.2.

## Adding coverage

Put a hook test beside its source in `hooks/src/__tests__/<name>.test.ts`, spawn the committed bundle, and assert its host output and state. Use `createSandbox` for every test. For a new hook surface, add cases to the conformance harness and update the [coverage inventory](gate-coverage-matrix.md).
