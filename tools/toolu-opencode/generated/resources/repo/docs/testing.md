# Testing with bun test

toolu uses `bun test` for its hook, tooling, and conformance suites ([epic #247](https://github.com/Falconiere/toolu/issues/247)). The quality gate is `cargo xtask gate`. `bun run test:unit`, `bun run test:conformance`, and `bun run test:rust-conformance` stay product suites.

CI runs that gate in the `rust` job. A docs-only change runs `cargo xtask gate --only context-budget --only portable-core --only ci-paths` and `cargo xtask check-markdown-cli`. See the CI section of AGENTS.md. The shell-analysis cold-start budget is hard on macOS arm64 or with `TOOLU_LATENCY_ENFORCE=1`, and report-only on Linux. See [conformance-report.md](conformance-report.md) for measurements against the `v7.2.0` Bash baseline.

OpenCode acceptance (`bun run test:opencode`) runs in its own CI jobs, `opencode (ubuntu-latest)` and `opencode (macos-latest)` ([#362](https://github.com/Falconiere/toolu/issues/362)). It installs the pinned `opencode-ai` CLI and drives it in isolated profiles against a scripted loopback provider, through every live check, regression control and budget. They run when the change touches the `opencode` path group. The required `typescript` status fails when a needed job fails or is cancelled, and when a job is skipped although its group is on. `bun run test:portable-core` runs `bun run check:opencode-host` and `bun run check:opencode-docs`, which check the committed evidence; see [opencode-host-contract.md](opencode-host-contract.md). `bun run check:opencode-docs` and the OpenCode smokes stay on Bun.

A test spawns the real thing: a hook bundle, `git`, `npm`, or a host CLI. It runs against real temp repositories and config roots. There are no mocks (AGENTS.md).

Shared helpers live in `@toolu/conformance/harness/*` (`tools/toolu-conformance/src/harness/`):

| Module | What it gives you |
|---|---|
| `sandbox` | `using sb = createSandbox({ git: true })`: a temp `project` (optionally a git repo with one commit), `home`, and `codexHome`. Helpers are `path`/`write`/`read`/`git`, plus `writeConfig(host, "project" \| "user", config)` for `toolu.config.json`. The tree is removed when the scope ends, even if the test throws. |
| `spawn` | `run(argv, { cwd, env, stdin, timeoutMs })` captures stdout, stderr, exit code, `durationMs` and `timedOut`. A timeout kills the whole process group. A missing binary resolves with exit 127. Host-session variables (`CLAUDE_*`, `CODEX_*`, `TOOLU_*`, `PLUGIN_ROOT`, …) never leak in, and an `env` value of `undefined` unsets a key. `runHook({ host, sandbox, pluginRoot, bundle \| argv, stdin })` spawns the real bundle as the host would. It sets `CLAUDE_PLUGIN_ROOT` / `PLUGIN_ROOT` / `CODEX_HOME` / `TOOLU_HOST_OVERRIDE` and runs with cwd at the project. |
| `hosts` | `readHostOutcome(host, event, result)` validates a hook's output against that host's contract and returns `{ effect: allow \| deny \| ask, reason?, context? }`. Claude and Codex use `hookSpecificOutput`, and Codex has no `ask`. Cursor answers with `permission` and an empty reply is invalid. `readOpencodeOutcome` reads the effect of an OpenCode permission event. A violation throws `HostOutputError`. |
| `fixtures` | Builders: `editFixture`, `writeFixture`, `patchFixture` (multi-file `apply_patch`), `bashFixture`, `mcpFixture`, `sessionFixture`, `promptFixture`, `postToolFixture`. `toStdin(host, fixture, { cwd })` renders the host's stdin: Codex receives edits as `apply_patch`, and Cursor gets `beforeShellExecution` / `beforeMCPExecution` / `preToolUse`. `toOpencodePermission` builds the in-process event. |
| `timing` | `measureLatency(once, { runs, warmup })` samples sequentially and reports p50/p95. `assertLatencyBudget(candidate, baseline, 5)` checks the cold-start delta measured in the same run when enforcement is enabled. |
| `entry-command` | The one place a test names a committed hook entry. `bundlePath(root, entry)` is `hooks/dist/<entry>.js` under `root`; `entryArgv(plugin, entry)` runs it with Bun; `publishedArgv(plugin, entry)` runs a skill CLI bundle by path through its shebang, as its published symlink does; `launchedArgv({ plugin, event, entry })` runs it through its `hooks.json` launcher under `sh -c`; `resolveEntryCommand` is the resolver behind them. Each returns the selected Rust command instead under `TOOLU_IMPL` (below). `implementationTag(plugin, entry)` is a test-name suffix that names the Rust run. |
| `startup` | SessionStart startup hooks run the way a host runs them: `runStartupHook(pluginRoot, entry, sb, env)` executes the plugin's real `hooks.json` launcher under `sh -c`; `startupEnv(host, sb, pluginRoot)` / `startupRoot` give the Claude or Codex environment and config root. `publishedCliSuite(spec)` registers the shared cases for a plugin that publishes a Bun CLI at a stable config-root path (both hosts with and without credentials, stale link, user file, Bun off `PATH`, no Bun, missing bundle). |

The parity case records and golden captures live in `fixtures/`; `fixtures/README.md` documents their shapes. The TypeScript suites load these committed JSON files at runtime; Rust ports read the same records. `index.json` records the original case names and `bun run tooling/src/check-fixture-inventory.ts` rejects missing, extra or renamed cases. The shared `json-cases` harness validates case envelopes, expands explicitly tagged sandbox paths, and runs a bounded set of real setup actions. Add a case to its JSON file, record its name in `index.json`, and the corresponding suite discovers it without changing a TypeScript case table. The three tooling-only overlay trees remain under `tooling/fixtures/` until #439.

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

Any other value, an empty item, a duplicate or a path separator inside a name is an error, never a silent fallback to Bun. The binary is `toolu` in `TOOLU_RUST_BIN_DIR`, or `target/release/toolu` at the repository root when that is unset. A selected entry runs `toolu hook <entry>` for the core `toolu` plugin and `toolu <plugin> hook <entry>` for the others, with the same stdin, cwd and environment the bundle gets. The two skill CLIs the binary serves as namespace verbs are the exception ([#421](https://github.com/Falconiere/toolu/issues/421)): `toolu/plan-ledger` runs `toolu ledger` and `toolu/verdict` runs `toolu ledger verdict`, each followed by the CLI's own arguments. A test that runs Markdown commands puts `installTooluShim(dir)` first on its `PATH`: that `toolu` sends `toolu ledger …` through the same seam, and fails with 127 on any other command. A selected binary that is missing, not a file or not executable fails the test at setup and names the selector and the expected path. The pre-tools case suites put `implementationTag` in their test names, so `TOOLU_IMPL=rust:toolu/pre-tools bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-golden.test.ts` reports a failing case as `… [rust:toolu/pre-tools]`.

Test files reach a bundle only through `entry-command`: `bundle-references.test.ts` fails on a test that spells the `hooks/dist` path (or `../dist/` from `hooks/src`) or calls `launcherCommand` itself, except for the listed files whose subject is the bundle or launcher text (packaging, drift, the launcher, and the OpenCode bootstrap, which spawns bundles in production code).

`test:conformance` runs four suites. `protected-files` and `spaces-cwd` dispatch `toolu/pre-tools` and take the Rust route when it is selected; `bootstrap-readiness` (OpenCode's missing-bundle diagnosis) and `surface-drift` (generated TypeScript) keep their checks in both modes.

`fixtures/ledger/cases.json` is the ledger's shared golden: `ledger-cases.test.ts` replays it through the seam, and `crates/cli/tests/ledger_cases.rs` replays it against the binary.

`fixtures/rust-ported.json` lists the ported entries, `{ "entries": ["<plugin>/<entry>", ...] }`, and a port adds its entry there. `bun run test:rust-conformance` reads it: an empty list is a no-op, otherwise it runs `cargo build --release --locked --bin toolu` (`CARGO` overrides the cargo executable), then `test:unit` and `test:conformance` with `TOOLU_IMPL=rust:<entries>`. CI runs it in the `rust-conformance` job, which installs the Rust toolchain only when the list is not empty.

A port also has a resource budget ([#410](https://github.com/Falconiere/toolu/issues/410), [resource-budgets.md](resource-budgets.md)). `bun run bench:hooks --assert` measures every entry in `fixtures/rust-ported.json` as Rust and fails when one is over its p50 budget in `benchmarks/hook-budgets.json`. It also fails when a ported entry has no budget, so a port adds its budget there too. The `hook-bench` CI job runs it on Linux; macOS only reports.

The bench and its end-to-end tests need the native measurer (`cargo xtask measure`), so they need the Rust toolchain. Test files named `*.native.test.ts` hold such tests: `test:unit` ignores them (`--path-ignore-patterns`), and the `hook-bench` job runs `tooling/src/benchmarks/__tests__/hook-resources.native.test.ts` before the bench.

Epic-orchestrator's `report.test.ts` and `epic-watch.test.ts` test the TypeScript scripts' own behavior and stay as they are: #434 (the resident engine) and #435 (worker reports and actions) replace them with Rust black-box tests, so they have no `toolu epic` mapping here.

## Concurrency rule

bats ran files in parallel and tests within a file serially, because suites shared per-file state. `bun test` suites follow a stronger rule: **files in parallel, tests concurrent.**

- Write every test as a top-level `test.concurrent(...)`. Where a required tool may be absent, use `test.concurrent.skipIf(NO_TOOL)(...)`. CI installs every such tool.
- Each test owns its temp directories through `createSandbox`. Nothing is shared at module scope.
- Never call `process.chdir` and never write `process.env`. Pass `cwd` and `env` to `run` instead.
- Latency measurements are the exception: they time one process at a time, so a timing test stays a plain `test`.

The suites pass under `bun test --parallel --concurrent --timeout 60000`. Every `bun test` in the package scripts passes `--timeout 60000`, because real subprocesses under concurrency outrun bun's 5 s default on a loaded CI runner. Pass it too when you run several suite files by hand.

On macOS, put Homebrew OpenSSL 3 before `/usr/bin` on `PATH` when running the HTTPS fixture: `PATH=/opt/homebrew/bin:$PATH`. Apple's `/usr/bin/openssl` is LibreSSL and its generated EC key fails to load in Bun 1.4.2.

## Adding coverage

Put a hook test beside its source in `hooks/src/__tests__/<name>.test.ts`, spawn the committed bundle, and assert its host output and state. Use `createSandbox` for every test. For a new hook surface, add cases to the conformance harness and update the [coverage inventory](gate-coverage-matrix.md).
