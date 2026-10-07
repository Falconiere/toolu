# toolu-engine: registry runner and dispatch — Plan

**Date:** 2026-10-07   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-07-toolu-engine-dispatch-design.md   **Topic:** #418. Port `@toolu/core/dispatch` and `@toolu/core/registry` to `crates/core/engine`: compiled-in rules behind manifests, `.sh` executables with a deadline, a batched Bun bridge for `.js`, gating, shadowing, the Codex prune and command detection. Also wire `toolu hook pre-tools` and `toolu hook post-tools`.

## Evidence and approach

The spec decides the design. The brainstorm records the Jev calls: CLI wiring 0.58, batched bridge 0.88, a timeout reported as `exited 124` 0.98, the manifest version line 0.64, manifest shadowing 0.79.

**Port sources:**
- `packages/toolu-core/src/dispatch/{dispatch,dispatch-walk,dispatch-output,dispatch-bash,dispatch-context}.ts`;
- `packages/toolu-core/src/registry/{registry-run,registry-list,registry-gate,registry-prune,registry-paths}.ts`;
- `packages/toolu-core/src/detect/detect-git.ts`.

**Unit-case sources:** `dispatch-native.test.ts`, `registry-run.test.ts`, `registry-paths.test.ts`, `detect-git.test.ts`.

**Reused:**
- `toolu-runtime`:
  - `env::Env`, `host::{detect, roots::Roots, snapshot::codex_plugin_installed}`;
  - `config::{load::load, read::enabled}`;
  - `registry::{parse_name, event_dir, manifest::read_manifest, rule::{Rule, RuleContext}}`;
  - `process::{run, Spec}`, `json::{jq_text, ordered::Ordered}`, `git::toplevel`.
- `toolu-protocol`: `decision::Decision`, `encode::encode`, `hook::{run_hook_io, Io, Reply, Raw}`, `normalized::NormalizedEvent`.
- `toolu-state`: `edit_records::normalize_edit_records`, `git::current_branch`, `js_order` (made `pub`).
- `toolu-shell`: `git::{runs_git_subcommand, push_targets}`.

**Conventions:**
- Integration tests use `#[path = "helpers/…"] mod …;` helpers returning `Res<T>`, following `crates/core/state/tests`.
- The shared golden is captured once by a scratch script that imports the TypeScript harness (#413–#415 practice). A Bun test and a Rust test both assert it.
- `docs/toolu` is gitignored, so design docs are force-added.

**Probes run:**
- `bun -e` with top-level `await Bun.stdin.text()` and `await import(<abs path>)` works on Bun 1.4.2.
- `Spec` is only built through `Spec::new`, so adding a field is source-compatible.
- `ExitCode: PartialEq`, as used in `crates/core/protocol/src/tests/hook_test.rs`.

**Constraints:**
- Rule 14: only `toolu_runtime::process` spawns, and `src` reads no `std::env`.
- 300 code lines per file, 50 per function, cognitive complexity 15.
- `toolu-engine` coverage floor 90%. No mocks: sentinel executables log and `exec` the real tool.
- Comemory notes:
  - `4a085d57`: a plan-ledger red in 0 s with empty evidence means admission was refused, so retry.
  - `be52369e` and `5c3e3c0f`: root-host Bun baseline failures; `capsh` drops for the full Bun gate.
- `cargo` must be `$HOME/.cargo/bin/cargo`, so every check sets `PATH`.
- Root-host reproduction of the CI `ts` job (AC-14): `capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'bun run test'`. Root ignores chmod, and the ptrace cap hides `/proc/<pid>/cwd` errors (comemory `be52369e`, `5c3e3c0f`). Long checks go through `bun <epic-orchestrator>/scripts/job.ts -- -- <cmd>`.

## Workstream summary

Runtime and state additions → the engine crate with its public registry API (listing, gating, prune) → the dispatch core, Bun bridge included, since the walk cannot run `.js` entries without it, so every intermediate build is warning-free → command detection → shared golden (TypeScript harness, capture, Bun test, Rust test) → live quality-bundle A/B → CLI wiring → the `measure` benchmark check → docs → full gate → delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "runtime-state-additions",
    "title": "toolu-runtime: process::Spec gains `wait: Wait` (Group default; Streams = child exited and both streams closed, group left alone unless the deadline passes), invocation::current_dir(), Rule::applies provided method (default true) and the EditSplit doc fix; toolu-state: js_order public with recursive js_ordered(Ordered); unit tests for each",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-runtime --lib process::tests:: && t -p toolu-runtime --lib invocation::tests:: && t -p toolu-runtime --lib registry::rule::tests:: && t -p toolu-state --lib js_order::tests::",
    "ac_refs": [
      "AC-3",
      "AC-9",
      "AC-1"
    ],
    "paths": [
      "crates/core/runtime/src/process.rs",
      "crates/core/runtime/src/tests/process_test.rs",
      "crates/core/runtime/src/invocation.rs",
      "crates/core/runtime/src/tests/invocation_test.rs",
      "crates/core/runtime/src/registry/rule.rs",
      "crates/core/runtime/src/registry/tests/rule_test.rs",
      "crates/core/state/src/js_order.rs",
      "crates/core/state/src/lib.rs",
      "crates/core/state/src/tests/js_order_test.rs"
    ],
    "input": "a real `sh -c 'sleep 30 >/dev/null 2>&1 & echo started'` child (Streams returns with `started` and no timeout; Group runs to the deadline); a Streams child that keeps stdout open past the deadline is terminated; current_dir equals the test cwd; a Rule without applies() reports true; JSON with integer-like keys at depth 3 (and 4294967295, which is not an index) reorders like JSON.parse",
    "model": "inherit"
  },
  {
    "id": "registry-gate-prune",
    "title": "toolu-engine crate skeleton (Cargo deps: toolu-protocol, toolu-runtime, toolu-shell, toolu-state, serde, serde_json; dev tempfile) with the public registry API: registry.rs (Entry, Listing, list_dir: byte order, dotfiles hidden, files after symlinks, rejected un-namespaced), registry/gate.rs (plugin_presence → Installed, plugin_active; Claude record path chain, Codex snapshot, other hosts unknown, fail open), registry/prune.rs (Codex only; regular .js/.sh/.json of absent specs in both dirs; never symlinks or un-namespaced)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-engine --lib registry:: && t -p toolu-engine --test prune",
    "ac_refs": [
      "AC-11",
      "AC-4"
    ],
    "depends_on": [
      "runtime-state-additions"
    ],
    "paths": [
      "crates/core/engine/Cargo.toml",
      "crates/core/engine/src/lib.rs",
      "crates/core/engine/src/registry.rs",
      "crates/core/engine/src/tests/registry_test.rs",
      "crates/core/engine/src/registry/gate.rs",
      "crates/core/engine/src/registry/prune.rs",
      "crates/core/engine/src/registry/tests/",
      "crates/core/engine/tests/prune.rs",
      "crates/core/engine/tests/helpers/",
      "Cargo.lock"
    ],
    "input": "temp config roots with real files: a symlinked module, a dotfile, a directory named like a module, `noname.sh`, `__x.sh`, `a b@c__d.sh`; malformed, absent and valid installed_plugins.json (CLAUDE_PLUGINS_REGISTRY, TOOLU_CONFIG_DIR, CLAUDE_CONFIG_DIR, HOME chains); ready, indeterminate and missing Codex snapshots; a Claude host for prune",
    "model": "inherit"
  },
  {
    "id": "engine-core",
    "title": "engine dispatch core with the Bun bridge: gate.rs, builtins.rs, trace.rs, dispatch.rs (Phase, DispatchOptions, Dispatched, ModuleResult, dispatch_pre_tool/post_tool) and dispatch/{session,event,edits,walk,fold,output}.rs; registry/{manifests,phase,executable}.rs and registry/bridge.rs + bridge/{runner,bun}.rs. Covers payload parse (surrogates, depth fail-closed), config-first, session env, toolEvent/RuleContext, built-ins (Err → exited 1, stderr dropped), the two-phase registry (gating, .js-over-.sh and manifest shadowing, manifest order read→matcher→rule→applies, executables with Streams/124/127/139/truncation, the bridge with batches, the scaled deadline, isolation, no-Bun deny/advisory/marker), consume/settle, the synthetic Edit with the two-level fold, malformed replies, continue_post_blocks",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-engine --lib && t -p toolu-engine --test executable && t -p toolu-engine --test rules && t -p toolu-engine --test walk && t -p toolu-engine --test payload && t -p toolu-engine --test bridge && cargo xtask gate --only fmt --only clippy --only guardrails --only layers",
    "ac_refs": [
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-7",
      "AC-8",
      "AC-9",
      "AC-15"
    ],
    "depends_on": [
      "registry-gate-prune"
    ],
    "paths": [
      "Cargo.lock",
      "clippy.toml",
      "crates/core/engine/src/",
      "crates/core/engine/tests/bridge.rs",
      "crates/core/engine/tests/executable.rs",
      "crates/core/engine/tests/helpers/",
      "crates/core/engine/tests/payload.rs",
      "crates/core/engine/tests/rules.rs",
      "crates/core/engine/tests/walk.rs",
      "rustfmt.toml",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "real bash .sh modules (env and stdin echo, exit 2/3, exit 0 with stderr, kill -SEGV $$, sleep past a 1 s deadline, trap '' TERM then exit 2, a backgrounder that then prints an advisory, >8 MiB output with and without exit 2, PATH without bash); counting test Rules (applies/run counters) and Gates (decision/Err); manifests (Edit/Write/*/mcp__* matchers, version 2, unknown field, unknown rule with fitting and non-fitting matcher); real ESM .js modules on real Bun 1.4 (allow/advisory/deny/throw/contract mismatch/invalid decision/not a module/process.exit(3)/infinite loop); a sentinel bun wrapper logging spawns; no Bun anywhere; two post calls with one session_id, a missing session_id, a file where the marker dir belongs; real Bash/Read/Write/Edit/apply_patch payloads (add/move/delete), a lone \\ud800 in a Bash command, a valid pair, a high surrogate followed by a non-low escape, a lone low surrogate, a literal \\\\ud800, nesting at and past serde's limit, hooks.pre-tools false",
    "model": "inherit"
  },
  {
    "id": "detect",
    "title": "engine detect.rs: is_git_push/is_git_commit (unknown → false), push_target_root (-C chain replayed from cwd, existing dir required, then cwd toplevel, project root, cwd), push_target_branch (current_branch unless HEAD or empty, else first push destination)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-engine --test detect && t -p toolu-engine --lib detect:: && cargo xtask gate --only fmt --only clippy --only guardrails --only layers",
    "ac_refs": [
      "AC-12"
    ],
    "depends_on": [
      "registry-gate-prune"
    ],
    "paths": [
      "clippy.toml",
      "crates/core/engine/src/detect.rs",
      "crates/core/engine/src/lib.rs",
      "crates/core/engine/src/tests/detect_test.rs",
      "crates/core/engine/tests/detect.rs",
      "crates/core/engine/tests/helpers/",
      "rustfmt.toml",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "real git repos: worktree add --detach into a dir with a space, branch feat/x, a detached HEAD, an unborn repo, a non-repo dir, dynamic -C \"$D\", a missing -C dir, and the five strip_heredocs and dynamic-name inputs from detect-git.test.ts",
    "model": "inherit"
  },
  {
    "id": "dispatch-golden",
    "title": "fixtures/dispatch/cases.json (~50 cases) captured once from the TypeScript dispatcher by a scratch script; packages/toolu-core/src/dispatch/__tests__/{dispatch-fixture-harness.ts,dispatch-fixture.test.ts}; crates/core/engine/tests/dispatch_fixture.rs reproducing every case byte for byte; fixtures/index.json suite `dispatch-cases`; fixtures/README.md row and counts",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-engine --test dispatch_fixture && o=$(bun test packages/toolu-core/src/dispatch/__tests__/dispatch-fixture.test.ts 2>&1) && printf '%s' \"$o\" | grep -Eq ' [1-9][0-9]* pass' && bun run tooling/src/check-fixture-inventory.ts",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-4",
      "AC-10"
    ],
    "depends_on": [
      "engine-core"
    ],
    "paths": [
      "fixtures/dispatch/cases.json",
      "fixtures/index.json",
      "fixtures/README.md",
      "packages/toolu-core/src/dispatch/",
      "crates/core/engine/tests/dispatch_fixture.rs",
      "crates/core/engine/tests/helpers/",
      "crates/core/engine/src/"
    ],
    "input": "sandbox git project, home and codex home; real bash/jq .sh modules, real ESM .js modules; Bash/Edit/Write/apply_patch payloads, invalid-JSON and trailing-newline stdin, the config switch for both hooks, Claude installed_plugins.json (valid and unreadable) and Codex snapshots",
    "model": "inherit"
  },
  {
    "id": "quality-bridge",
    "title": "live A/B: packages/toolu-core/src/dispatch/__tests__/dispatch-cli.ts (request JSON in → dispatchPostTool result JSON out) and crates/core/engine/tests/quality_bridge.rs running the committed ts-quality bundle as ts-quality@toolu__ts-quality.js in two identical TS-project sandboxes through TypeScript and Rust, for a violating and a clean .ts edit; compare stdout/stderr/exit and the gate entry with updatedAt and sandbox paths normalized",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-engine --test quality_bridge",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "engine-core"
    ],
    "paths": [
      "packages/toolu-core/src/dispatch/__tests__/dispatch-cli.ts",
      "crates/core/engine/tests/quality_bridge.rs",
      "crates/core/engine/tests/helpers/",
      "plugins/ts-quality/hooks/dist/post-tool-use.js",
      "crates/core/engine/src/"
    ],
    "input": "plugins/ts-quality/hooks/dist/post-tool-use.js registered as ts-quality@toolu__ts-quality.js; a git project with tracked tsconfig.json and bun.lock; a .ts file with a violation and a clean .ts file, each edited through an Edit payload",
    "model": "inherit"
  },
  {
    "id": "cli-wiring",
    "title": "toolu-hub tool_hook(Phase re-exported by the hub as toolu_hub::Phase, so crates/cli gains no engine dependency, payload: io::Result<String>, plugin_root) → Outcome (Env::process, invocation::current_dir, lib dir from plugin root, engine builtins + hub RULES, run_hook_io over in-memory streams with a failing reader for Err, one trailing newline stripped, None when empty) and crates/cli/src/hook.rs routing `toolu hook pre-tools|post-tools`; crates/cli/tests/tool_hooks.rs byte-exact black-box tests; contract snapshot and docs-cli unchanged",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-cli --test tool_hooks && t -p toolu-cli --bin toolu && t -p toolu-hub --lib && t -p toolu-cli --test contract && cargo xtask gate --only docs-cli && cargo xtask gate --only fmt --only clippy --only guardrails --only layers",
    "ac_refs": [
      "AC-13",
      "AC-5"
    ],
    "depends_on": [
      "engine-core"
    ],
    "paths": [
      "Cargo.lock",
      "clippy.toml",
      "crates/cli/src/hook.rs",
      "crates/cli/src/tests/hook_test.rs",
      "crates/cli/tests/fixtures/",
      "crates/cli/tests/helpers/",
      "crates/cli/tests/tool_hooks.rs",
      "crates/toolu/",
      "docs/cli/",
      "rustfmt.toml",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "the built toolu binary run by assert_cmd with real stdin: a .sh exit-2 module with newline-terminated stderr, one whose stderr lacks the final newline, one printing only \\n to stderr, a block module for post-tools, a Read with an empty registry, a manifests-only registry with sentinel bash/bun first on PATH, a non-UTF-8 payload (exit 2), and hooks.pre-tools false",
    "model": "inherit"
  },
  {
    "id": "measure-read",
    "title": "AC-5 benchmark check: the release toolu run as `toolu hook pre-tools` under the release `cargo xtask measure`, with a Read payload, a manifests-only registry (Write|Edit matcher) and sentinel bash/bun first on PATH, in an environment cleared of host variables: exit 0, max RSS within the 6 MiB pre-tools budget, no sentinel log; prints the report",
    "check": "set -e; export PATH=\"$HOME/.cargo/bin:$PATH\"; root=$PWD; s=$(mktemp -d); cargo build -q --release -p toolu-cli; cargo build -q --release -p xtask; d=\"$s/home/.claude/toolu/pre-tools.d\"; mkdir -p \"$d\" \"$s/bin\" \"$s/proj\"; printf '%s' '{\"version\":1,\"spec\":\"x@t\",\"name\":\"r\",\"event\":\"tool/pre\",\"matcher\":\"Write|Edit\"}' > \"$d/x@t__r.json\"; for b in bash bun; do printf '#!/bin/sh\\necho %s >> \"%s/spawned\"\\nexit 1\\n' \"$b\" \"$s\" > \"$s/bin/$b\"; chmod +x \"$s/bin/$b\"; done; printf '%s' '{\"hook_event_name\":\"PreToolUse\",\"session_id\":\"m\",\"tool_name\":\"Read\",\"tool_input\":{\"file_path\":\"/etc/hosts\"}}' > \"$s/read.json\"; cd \"$s/proj\"; env -i HOME=\"$s/home\" PATH=\"$s/bin:/usr/bin:/bin\" TOOLU_CONFIG_DIR=\"$s/home/.claude\" \"$root/target/release/xtask\" measure --out \"$s/m.json\" -- \"$root/target/release/toolu\" hook pre-tools < \"$s/read.json\"; cd \"$root\"; test ! -e \"$s/spawned\"; jq -e '.exitCode == 0 and .maxRssBytes <= 6291456' \"$s/m.json\"; jq -c . \"$s/m.json\"",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "cli-wiring"
    ],
    "paths": [
      "crates/",
      "Cargo.lock",
      "benchmarks/hook-budgets.json"
    ],
    "input": "the release toolu binary, a Read PreToolUse payload, a manifests-only pre-tools.d, sentinel bash/bun that record any spawn, `env -i` so no CLAUDE_*/TOOLU_*/CODEX_* variable of the session leaks in",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/registry.md native-engine section (walk and two phases, manifests and applies, exited 124, Bun bridge and deadline/hang note, manifest shadowing, payload rules), docs/detect.md detection move, AGENTS.md key-files rows (engine; cli pre-tools/post-tools native), fixtures/README.md row and counts, fixtures/index.json",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; bun run tooling/src/check-fixture-inventory.ts && cargo xtask check-markdown-cli && bun run check:retired-plugins && grep -q 'Native engine (Rust)' docs/registry.md && grep -q 'exited 124' docs/registry.md && grep -q 'Bun bridge' docs/registry.md && grep -q 'toolu_engine::detect' docs/detect.md && grep -q 'crates/core/engine/src/lib.rs' AGENTS.md && grep -q 'pre-tools.*post-tools.*native\\|native.*pre-tools.*post-tools' AGENTS.md && grep -q 'dispatch-cases\\|dispatch/cases.json' fixtures/README.md",
    "ac_refs": [
      "AC-14"
    ],
    "depends_on": [
      "dispatch-golden",
      "cli-wiring",
      "detect",
      "quality-bridge",
      "measure-read"
    ],
    "paths": [
      "docs/registry.md",
      "docs/detect.md",
      "AGENTS.md",
      "fixtures/README.md",
      "fixtures/index.json"
    ],
    "input": "the shipped behaviour of the steps above",
    "model": "inherit"
  },
  {
    "id": "full-gate",
    "title": "cargo xtask gate (fmt, clippy, guardrails, layers, coverage ≥ 90% for toolu-engine, jscpd, docs-cli, cli-compat, unused-pub, hooks) with the PR title, and `bun run test` under the root-host capability drops",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(engine): registry runner and dispatch in Rust (#418)' && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'export PATH=\"$HOME/.cargo/bin:$PATH\"; bun run test'",
    "ac_refs": [
      "AC-14"
    ],
    "depends_on": [
      "docs"
    ],
    "paths": [
      "crates/",
      "Cargo.toml",
      "Cargo.lock",
      "fixtures/",
      "packages/",
      "docs/",
      "AGENTS.md",
      "tooling/"
    ],
    "input": "the whole workspace",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/runtime/src/{process.rs,invocation.rs,registry/rule.rs}` and their `tests/*_test.rs`
- `crates/core/state/src/{js_order.rs,lib.rs}`, `crates/core/state/src/tests/js_order_test.rs`
- `crates/core/engine/Cargo.toml`, `crates/core/engine/src/{lib,gate,builtins,trace,dispatch,detect,registry}.rs`
- `crates/core/engine/src/dispatch/{session,event,edits,walk,fold,output}.rs`
- `crates/core/engine/src/registry/{gate,prune,manifests,phase,executable,bridge}.rs`, `crates/core/engine/src/registry/bridge/{runner,bun}.rs`
- `crates/core/engine/src/**/tests/*_test.rs`, `crates/core/engine/tests/{prune,executable,rules,walk,payload,bridge,dispatch_fixture,quality_bridge,detect}.rs`, `crates/core/engine/tests/helpers/*.rs`
- `crates/toolu/{Cargo.toml,src/lib.rs,src/tool_hook.rs,src/tests/tool_hook_test.rs}`, `crates/cli/src/hook.rs`, `crates/cli/tests/tool_hooks.rs`
- `fixtures/dispatch/cases.json`, `fixtures/index.json`, `fixtures/README.md`
- `packages/toolu-core/src/dispatch/__tests__/{dispatch-fixture.test.ts,dispatch-fixture-harness.ts,dispatch-cli.ts}`
- `docs/registry.md`, `docs/detect.md`, `AGENTS.md`
- `docs/toolu/{brainstorms,specs,plans}/2026-10-07-toolu-engine-dispatch*.md`, which are force-added because `docs/toolu` is gitignored

## Verification

- **End to end:** the shared golden passes in both implementations. The live ts-quality A/B matches. `toolu hook pre-tools` blocks on a `.sh` exit 2 and lets a `Read` through with no spawn. The `cargo xtask measure` reading of `toolu hook pre-tools` (release) with a `Read` payload goes in the PR body.
- **Failure and boundary:** timeouts (124) and crashes (139); a TERM-trapping module that exits 2 is not a deny; Bun absent before a tool (deny) and after it (one advisory per session); manifest problems; malformed patches; the config switch; fail-open gating; prune leaving symlinks alone.
- **Docs:** registry, detect, `AGENTS.md`, and the fixtures README and index updated in the `docs` step and gated by the fixture inventory and the Markdown–CLI check.

## Delivery

1. Before each push: `git fetch origin`, and rebase on `origin/main` if it moved. Then re-run the affected step checks.
2. Commit scoped work with `feat(engine): …` (or `test`/`docs` for the parts that are only tests or docs). Stage the declared paths explicitly, and force-add the `docs/toolu` design docs. Squash-merge makes the PR title the commit of record, so the title is `feat(engine): registry runner and dispatch in Rust (#418)`.
3. Run `bun plugins/toolu/hooks/dist/plan-ledger.js run docs/toolu/plans/2026-10-07-toolu-engine-dispatch.md --verify`. Every step must be fresh-green against the final diff.
4. Run `toolu-review:review` on the committed branch diff, which records v2 push-review state over every changed file. Then run `bun plugins/toolu/hooks/dist/verdict.js status`, which must report `overall: ready`. Any change after step 3 (a review fix or a rebase) repeats step 3 and the review before the push.
5. Push `feat/418-toolu-engine-registry-runner-and`. Create the PR against `main` with that title and a body that starts with `Closes Falconiere/toolu#418` and `Part of Falconiere/toolu#402`. The body includes the `measure-read` report line and the test evidence. Verify the PR number and its head and base branches. Then hand off to `pr-babysit:babysit`.
