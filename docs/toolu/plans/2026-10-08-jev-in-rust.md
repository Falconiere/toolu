# Jev in Rust — Plan

**Date:** 2026-10-08   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-08-jev-in-rust-design.md   **Topic:** #430. `toolu jev` on the core client, native hooks, shim, Bun sources deleted last.

## Evidence and approach

The approved spec is the contract. Brainstorm decisions: clap verbs; exit codes 0, 1, 2, 64, 69, 75 (HTTP body exits 1, timeout exits 75); the mandate says `toolu jev` unless the published path is a kept user file. Spec review required the Rust tests to pass before `hooks/src` and `hooks/dist` are deleted. No further classification: the CI build belongs in the switch step, because `test:ts` and `test:opencode` execute jev's real SessionStart and those jobs do not build `toolu` today.

**Reused:**
- `toolu_jev_client::{Jev, Config, Question, Questions, State, Error, Reply}`
- `toolu_runtime::startup::{publish, session_context, render_hook_output}` and `Roots::new`
- `toolu_runtime::env::Env`, `toolu_runtime::process` (`native_toolu_on_path`)
- `toolu_http_test_support::Fixture` (dev-dependency; layers ignore dev-deps)
- `toolu_protocol::stdin::read_stdin` and `toolu_protocol::exit::Exit`
- `cargo xtask print-hook` and `check-hooks`
- Oracle files, read then deleted: `plugins/jev/hooks/src/__tests__/jev.test.ts`, `session-start.test.ts`, `user-prompt-submit.test.ts`, `opencode.test.ts`

**Constraints:**
- Prefix cargo with `PATH="$HOME/.cargo/bin:$PATH"`.
- Plugin `src` does not call `std::env`, `std::process::Command`, `std::io::stdout`, or `std::io::stderr`.
- `Ctx` literals in this repo use `..Ctx::default()`, so a new `stdin` field with `Default` only needs an explicit value in `ctx_of`.
- `docs/toolu` is gitignored and force-added, as in #413–#417.
- 300 code lines per file, 50 per function, a wired test per module that has a function.

## Workstream summary

Runtime notice, then the jev CLI and its hook functions while the TypeScript oracle still exists, then CLI dispatch, then Markdown and `docs/cli`, then the shim, native `hooks.json`, deletion, OpenCode expectations, and the CI build, then the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "native-advice",
    "title": "toolu-runtime startup: native.rs ports nativeTooluAdvice (silent when command -v toolu is native, else one install or absolute-path line, once per session_id else TOOLU_SESSION_ID, FNV-1a hex filename, no new crate)",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib startup::native::tests 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": ["AC-6"],
    "paths": [
      "crates/core/runtime/src/startup.rs",
      "crates/core/runtime/src/startup/native.rs",
      "crates/core/runtime/src/startup/tests/native_test.rs",
      "crates/core/runtime/src/process/commands.rs",
      "packages/toolu-core/src/startup/native-toolu.ts"
    ],
    "input": "temp PATH with a real executable whose --hook-protocol prints 1; empty PATH; two calls with session_id s1; a call with no session id",
    "model": "sonnet"
  },
  {
    "id": "jev-cli",
    "title": "toolu-http parses a PEM file to DER; Jev::from_env uses NODE_EXTRA_CA_CERTS only when Config has no test root. toolu-jev: clap verbs, call.rs, present.rs, public execute and needs_stdin; tests/cli.rs ports jev.test.ts (status 22 becomes exit 1, status 28 becomes exit 75)",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-jev --test cli 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-jev --lib 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-http --lib 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-8"],
    "paths": [
      "crates/jev/",
      "crates/core/http/",
      "crates/core/jev/",
      "crates/http-test-support/",
      "plugins/jev/hooks/src/__tests__/jev.test.ts",
      "plugins/jev/hooks/src/jev.ts",
      "plugins/jev/hooks/src/jev/"
    ],
    "input": "Fixture TLS origin and CONNECT proxy; noul body {\"model\":\"jev-1.13.0\",\"answers\":{\"q\":{\"type\":\"noul\",\"noul\":0.92}},\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}; choice criteria billing=Payments and bare tech; 401 body {\"error\":\"bad key\"}; JEV_TIMEOUT=0; 408 then the noul body; Retry-After 61; a dropped connection; missing key; key with a newline; __proto__ option; ask from stdin",
    "model": "sonnet"
  },
  {
    "id": "jev-hooks",
    "title": "toolu-jev session.rs, prompt.rs, and check.rs, tested against a temp plugin root that contains the shim: mandate toolu jev or the quoted user path, OpenCode compact is silent, Claude compact still speaks, trivial prompts stay silent, check-binary renders native_toolu_advice",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-jev --lib session::tests 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-jev --lib prompt::tests 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-jev --lib check::tests 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": ["AC-4", "AC-5", "AC-6"],
    "depends_on": ["native-advice", "jev-cli"],
    "paths": [
      "crates/jev/src/session.rs",
      "crates/jev/src/prompt.rs",
      "crates/jev/src/check.rs",
      "crates/jev/src/tests/session_test.rs",
      "crates/jev/src/tests/prompt_test.rs",
      "crates/jev/src/tests/check_test.rs",
      "plugins/jev/hooks/src/session-start.ts",
      "plugins/jev/hooks/src/user-prompt-submit.ts",
      "plugins/jev/hooks/src/__tests__/session-start.test.ts",
      "plugins/jev/hooks/src/__tests__/user-prompt-submit.test.ts",
      "plugins/jev/hooks/src/__tests__/opencode.test.ts"
    ],
    "input": "temp homes for Claude, Codex with a space in CODEX_HOME, and OpenCode TOOLU_HOST_OVERRIDE; TOOLU_CONFIG_DIR over the Codex root; source compact on both hosts; a regular file at jev.sh; a missing shim; an unwritable config dir; prompts rank these approaches, LGTM, a slash command, and a multiline task",
    "model": "sonnet"
  },
  {
    "id": "cli-wire",
    "title": "Ctx.stdin default None; dispatch reads stdin when toolu_jev::needs_stdin; hook.rs dispatches jev session-start, user-prompt-submit, and check-binary to toolu_jev and prefixes a SessionStart skew advisory; a cli test spawns the built toolu with the key unset",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-cli --bin toolu hook::tests 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-cli --test jev_cli 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-cli -p toolu-runtime -p toolu-hub -p toolu-epic-orchestrator && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": ["AC-2"],
    "depends_on": ["jev-cli", "jev-hooks"],
    "paths": [
      "crates/core/runtime/src/cli.rs",
      "crates/cli/src/dispatch.rs",
      "crates/cli/src/hook.rs",
      "crates/cli/src/tests/hook_test.rs",
      "crates/cli/tests/jev_cli.rs",
      "crates/jev/src/lib.rs"
    ],
    "input": "spawned target/debug/toolu jev noul -s STATE Urgent? with TYPESAFE_API_KEY unset; a jev SessionStart payload {\"source\":\"startup\"} through hook dispatch",
    "model": "sonnet"
  },
  {
    "id": "docs",
    "title": "Rewrite plugins/jev skill, problem-solving, evals README, and plugin README to toolu jev with no bun or hooks/dist; regenerate the OpenCode skill surface and docs/cli",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask docs-cli && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-markdown-cli && bun run check:opencode-surface && ! grep -R -n -E 'hooks/dist|\\bbun\\b' plugins/jev/skills plugins/jev/README.md tools/toolu-opencode/generated/skills/jev-jev",
    "ac_refs": ["AC-7"],
    "depends_on": ["cli-wire"],
    "paths": [
      "plugins/jev/skills/",
      "plugins/jev/README.md",
      "tools/toolu-opencode/generated/skills/jev-jev/",
      "docs/cli/"
    ],
    "input": "toolu jev --help and toolu commands --json after the verbs exist; the skill files as the markdown-cli scan input",
    "model": "sonnet"
  },
  {
    "id": "switch",
    "title": "Add plugins/jev/scripts/jev.sh, replace hooks.json with print-hook entries, delete hooks/src and hooks/dist, point OpenCode helpers at scripts/jev.sh, change the live test's bash line from Bun plus jev.sh to toolu jev and drop the --no-env-file expectation, set inventory hostMechanism to native and render the matrix, and build toolu before test:ts and test:opencode",
    "check": "set -e; test ! -e plugins/jev/hooks/src; test ! -e plugins/jev/hooks/dist; grep -q 'exec toolu jev' plugins/jev/scripts/jev.sh; PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-hooks; bun run tooling/src/gate-coverage-inventory.ts check; export PATH=\"$HOME/.cargo/bin:$PWD/target/debug:$PATH\"; cargo build -p toolu-cli; bun test --timeout 60000 tools/toolu-opencode/src/bootstrap/__tests__/startup-catalog.test.ts tools/toolu-opencode/src/bootstrap/__tests__/startup-ledger.test.ts tools/toolu-opencode/src/bootstrap/__tests__/startup-readiness.test.ts tools/toolu-opencode/src/bootstrap/__tests__/startup-isolation.test.ts tools/toolu-opencode/src/plugin/__tests__/jev-delivery.test.ts tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts",
    "ac_refs": ["AC-4", "AC-7"],
    "depends_on": ["jev-hooks", "docs"],
    "paths": [
      "plugins/jev/scripts/jev.sh",
      "plugins/jev/hooks/hooks.json",
      "plugins/jev/hooks/",
      "fixtures/gate-coverage/inventory.json",
      "docs/gate-coverage-matrix.md",
      ".github/workflows/tests.yml",
      "tools/toolu-opencode/src/bootstrap/__tests__/",
      "tools/toolu-opencode/src/plugin/__tests__/jev-delivery.test.ts",
      "tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts"
    ],
    "input": "the real plugins/jev tree after deletion; startup-catalog's helper map jev/jev.sh -> jev/scripts/jev.sh; a built toolu on PATH for the bootstrap subprocess",
    "model": "sonnet"
  },
  {
    "id": "full-gate",
    "title": "cargo xtask gate for this branch and the TypeScript gate that the jev and OpenCode edits reach",
    "check": "set -e; export PATH=\"$HOME/.cargo/bin:$PWD/target/debug:$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(jev): native toolu jev namespace and hooks (#430)'; bun run test:ts; TOOLU_LIVE_OPENCODE=1 bun test --timeout 180000 tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts",
    "ac_refs": ["AC-1", "AC-7"],
    "depends_on": ["switch", "cli-wire"],
    "paths": [
      "crates/",
      "plugins/jev/",
      "docs/cli/",
      "docs/gate-coverage-matrix.md",
      "fixtures/gate-coverage/inventory.json",
      ".github/workflows/tests.yml",
      "tools/toolu-opencode/"
    ],
    "input": "this branch against origin/main, with target/debug/toolu on PATH",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/runtime/src/startup/native.rs` and `crates/core/runtime/src/startup/tests/native_test.rs`; export from `startup.rs`.
- `crates/jev/Cargo.toml` and `crates/jev/src/{lib,cli,call,present,session,prompt,check}.rs` with `src/tests/*_test.rs`; `crates/jev/tests/cli.rs`.
- `crates/core/runtime/src/cli.rs` (`stdin: Option<String>`).
- `crates/cli/src/{dispatch,hook}.rs`, `crates/cli/src/tests/hook_test.rs`, `crates/cli/tests/jev_cli.rs`.
- `plugins/jev/scripts/jev.sh`, `plugins/jev/hooks/hooks.json`; delete `plugins/jev/hooks/src` and `plugins/jev/hooks/dist`.
- `plugins/jev/skills/jev/SKILL.md`, `references/problem-solving.md`, `evals/README.md`, `plugins/jev/README.md`.
- `tools/toolu-opencode/generated/skills/jev-jev/` and the bootstrap and jev-delivery tests that name `hooks/dist/jev.js`.
- `fixtures/gate-coverage/inventory.json`, `docs/gate-coverage-matrix.md`.
- `docs/cli/` from `cargo xtask docs-cli`.
- `.github/workflows/tests.yml` jobs `ts` and `opencode`.

## Verification

The loopback fixture proves AC-1, AC-3, and AC-8, including the failure rows (401 body, timeout, retry, missing key). Temp homes prove AC-4 and AC-5. A real executable on `PATH` proves AC-6. AC-7 is `check-hooks`, `check-markdown-cli`, `check:plugin-bundles` inside `test:ts`, and an absent `hooks/src`. AC-2 is also the spawned `toolu` binary.

Delivery, after `full-gate` is green, follows the execution reference:

1. Scoped `feat(jev):` commit. Force-add `docs/toolu` for this issue's brainstorm, spec, and plan.
2. `plan-ledger run` this file `--verify`.
3. `toolu-review:review` with version-2 state.
4. `verdict.js status` reporting `overall: ready`.
5. Push `feat/430-jev-in-rust`, open the PR (`Closes Falconiere/toolu#430`, `Part of Falconiere/toolu#402`), then `pr-babysit:babysit`.

## Deviations

- `native-advice`: `shell_toolu` takes the caller's `Env` so the probe does not see the process `PATH`. `native_toolu_on_path()` still uses `Env::process()`. The session-id fallback (`session_id`, else `TOOLU_SESSION_ID`) stays in the hook, as in `runNativeTooluCheck`.
- `jev-cli`: clap cannot count repeated `-o`/`-l` flags, so too few options, too few levels, and `ask - -s -` exit 1 from the client. Unknown flags and a missing `--state` stay clap usage (exit 64) in `crates/cli`. The command-tree snapshot, `docs/cli`, and the tests that named `jev planned` move in this step because that verb is gone.
