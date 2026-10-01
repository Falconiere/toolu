# Testing with bun test

toolu uses `bun test` for its hook, tooling, and conformance suites ([epic #247](https://github.com/Falconiere/toolu/issues/247)). `bun run test` runs the TypeScript gate: conventions, real-subprocess unit and conformance tests, portable-core and gate-coverage checks, final-removal checks, bundle and launcher drift, package/workspace checks, context budget, deterministic benchmarks, and the shell-analysis latency measurement.

The CI job is `typescript`. Its shell-analysis cold-start budget is hard on macOS arm64 or with `TOOLU_LATENCY_ENFORCE=1`, and report-only on Linux. See [conformance-report.md](conformance-report.md) for measurements against the `v7.2.0` Bash baseline.

A test spawns the real thing: a hook bundle, `git`, `npm`, or a host CLI. It runs against real temp repositories and config roots. There are no mocks (AGENTS.md).

Shared helpers live in `@toolu/conformance/harness/*` (`tools/toolu-conformance/src/harness/`):

| Module | What it gives you |
|---|---|
| `sandbox` | `using sb = createSandbox({ git: true })`: a temp `project` (optionally a git repo with one commit), `home`, and `codexHome`. Helpers are `path`/`write`/`read`/`git`, plus `writeConfig(host, "project" \| "user", config)` for `toolu.config.json`. The tree is removed when the scope ends, even if the test throws. |
| `spawn` | `run(argv, { cwd, env, stdin, timeoutMs })` captures stdout, stderr, exit code, `durationMs` and `timedOut`. A timeout kills the whole process group. A missing binary resolves with exit 127. Host-session variables (`CLAUDE_*`, `CODEX_*`, `TOOLU_*`, `PLUGIN_ROOT`, …) never leak in, and an `env` value of `undefined` unsets a key. `runHook({ host, sandbox, pluginRoot, bundle \| argv, stdin })` spawns the real bundle as the host would. It sets `CLAUDE_PLUGIN_ROOT` / `PLUGIN_ROOT` / `CODEX_HOME` / `TOOLU_HOST_OVERRIDE` and runs with cwd at the project. |
| `hosts` | `readHostOutcome(host, event, result)` validates a hook's output against that host's contract and returns `{ effect: allow \| deny \| ask, reason?, context? }`. Claude and Codex use `hookSpecificOutput`, and Codex has no `ask`. Cursor answers with `permission` and an empty reply is invalid. `readOpencodeOutcome` reads the effect of an OpenCode permission event. A violation throws `HostOutputError`. |
| `fixtures` | Builders: `editFixture`, `writeFixture`, `patchFixture` (multi-file `apply_patch`), `bashFixture`, `mcpFixture`, `sessionFixture`, `promptFixture`, `postToolFixture`. `toStdin(host, fixture, { cwd })` renders the host's stdin: Codex receives edits as `apply_patch`, and Cursor gets `beforeShellExecution` / `beforeMCPExecution` / `preToolUse`. `toOpencodePermission` builds the in-process event. |
| `timing` | `measureLatency(once, { runs, warmup })` samples sequentially and reports p50/p95. `assertLatencyBudget(candidate, baseline, 5)` checks the cold-start delta measured in the same run when enforcement is enabled. |
| `startup` | SessionStart startup hooks run the way a host runs them: `runStartupHook(pluginRoot, entry, sb, env)` executes the plugin's real `hooks.json` launcher under `sh -c`; `startupEnv(host, sb, pluginRoot)` / `startupRoot` give the Claude or Codex environment and config root. `publishedCliSuite(spec)` registers the shared cases for a plugin that publishes a Bun CLI at a stable config-root path (both hosts with and without credentials, stale link, user file, Bun off `PATH`, no Bun, missing bundle). |

A port test reads like this:

```ts
import { expect, test } from "bun:test";
import { join, resolve } from "node:path";
import { editFixture, toStdin } from "@toolu/conformance/harness/fixtures";
import { readHostOutcome } from "@toolu/conformance/harness/hosts";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { runHook } from "@toolu/conformance/harness/spawn";

const pluginRoot = resolve(import.meta.dir, "../../.."); // plugins/<name>
const bundle = join(pluginRoot, "hooks/dist/<entry>.js");

test.concurrent("protected files deny an .env edit on codex", async () => {
  using sb = createSandbox({ git: true });
  sb.writeConfig("codex", "project", { version: 1, gates: { protectedFiles: { mode: "block" } } });
  const stdin = toStdin("codex", editFixture(sb.path(".env"), "a", "b"), { cwd: sb.project });
  const res = await runHook({ host: "codex", sandbox: sb, pluginRoot, bundle, stdin });
  expect(readHostOutcome("codex", "PreToolUse", res).effect).toBe("deny");
});
```

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
