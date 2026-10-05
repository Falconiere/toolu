# Rust workspace: one `toolu` CLI, one crate per plugin

> Tracked as epic #402, which is the delivery tracker: its sub-issues and phase table supersede the order sketched here. The epic-orchestrator design is in [2026-10-04-epic-engine.md](2026-10-04-epic-engine.md): a resident engine, not a port of the watcher.

Brainstorm, 2026-10-04. The user decided to rebuild every toolu plugin in Rust, with a `crates/` workspace holding `crates/core/` and one crate per plugin, and then that the Rust code is one CLI which each plugin's Markdown (skills, commands, agents) runs. Motivation: hook memory and CPU (see [2026-10-04-hook-resource-diet.md](2026-10-04-hook-resource-diet.md) for the measurements). This file covers workspace shape only. How binaries reach Claude Code and Codex installs is still open (see "Open decision").

## Capsule

- **Outcome:** a Cargo workspace in which each plugin with code is one library crate under `crates/<plugin>/` that adds a namespace to a single `toolu` binary, the shared core is a few layered crates under `crates/core/`, and hooks run natively as `toolu hook <event>`. Prototype measurement: 2.8 MB RSS and under 10 ms per spawn, against 38 to 46 MB and 45 to 150 ms today.
- **Material defaults:**
  - A crate is not a binary. `crates/cli` builds the only binary, `toolu`, and every plugin crate is a library (Jev: 0.98 for one CLI over a binary per plugin). Rule plugins (ts-, python-, rust-quality and the ast-grep rules) are linked into the dispatcher.
  - Core is split into six layered crates, not one big crate.
  - `plugins/<name>/` stays the install tree: the skills, commands and agents that tell the agent which `toolu` command to run, `hooks.json`, `plugin.json` and a `bin/toolu` launcher. `crates/<name>/` is the source.
  - Migration is plugin by plugin, with today's black-box tests and JSON fixtures checking each Rust port.
- **Non-goal:**
  - Markdown-only plugins (brainstorm, delivery-flow) get no crate. Skills, commands and agents stay Markdown.
  - exa-search, context7, jira and agent-browser are deprecated and removed, not ported (see "Removed plugins").
- **Repository evidence:** the core import graph (below); inventory of 16 plugins and about 45k lines of source; 333 test files, 55 to 60% of them black-box spawns.
- **Risk:**
  - Distribution is undecided.
  - The ESM registry contract for third parties is dropped, which is a breaking change.
  - The gate file must stay byte-compatible while TypeScript and Rust hooks coexist.
  - The shell parser has to match unbash on the parity fixtures.
- **Handoff:** epic #402. "Open decision" is its sub-issue #411.

## Layout

```
toolu/
  Cargo.toml                 # [workspace], lockstep version, workspace deps and lints
  rust-toolchain.toml        # pinned toolchain
  crates/
    core/
      protocol/   toolu-protocol   # host payloads, NormalizedEvent, Decision, output encoders, policy. serde only
      runtime/    toolu-runtime    # host detection and roots, config (fail-closed), process, cli helpers, startup publish, registry types
      shell/      toolu-shell      # brush-parser, tree-sitter-bash fallback, ShellAnalysis, write targets. fuzzed
      http/       toolu-http       # ureq + rustls, HTTPS_PROXY, the REST wrapper
      state/      toolu-state      # gate file (locked, atomic), telemetry, edit records, diff sha, git facts from .git
      engine/     toolu-engine     # registry runner, dispatch, gates, ledger, resources, quality runner
    cli/               bin `toolu`          # the only binary; links every plugin crate; `toolu commands --json`
    toolu/             lib                  # `toolu hook …`, `toolu ledger …`, doctor/config/status, `serve --stdio`; links the rule crates
    ts-quality/        lib                  # rules only
    python-quality/    lib
    rust-quality/      lib
    ast-grep/          lib                  # nudge and byte-savings rules, `toolu ast-grep …`
    jev/               lib                  # `toolu jev …`
    statusline/        lib                  # `toolu statusline …`
    toolu-review/      lib                  # `toolu review …`
    pr-babysit/        lib                  # `toolu babysit …`
    epic-orchestrator/ lib                  # `toolu epic …`: engine, merge queue, status server, trackers
    plugins/           lib                  # `toolu plugins …`, the installer behind npx @toolu/plugins
    xtask/             `cargo xtask`        # build, checks, budgets, docs, release packaging
  fixtures/          # language-neutral JSON contracts shared by both test suites during migration
  packages/opencode-shim/   # thin TypeScript plugin that spawns `toolu serve --stdio`
  plugins/<name>/    # install trees: Markdown that runs `toolu …`, hooks.json on the native launcher, bin/toolu
```

Directory names are short and package names are prefixed (`toolu-*`), so nothing collides on crates.io if a crate is ever published.

## Core shape

The TypeScript import graph decides the layers.

| Module | Imports |
|--------|---------|
| host | decision, events |
| config | decision, host |
| shell | events (and the parser) |
| detect | host, shell |
| state | config, detect, host |
| registry | startup, state, … |
| startup | registry, state, … (a cycle with registry) |
| ledger | config, host, resources, state |
| gates | almost everything |

A single `core` crate would make every plugin crate compile the shell parser and TLS, whether it uses them or not. It would also hide the registry ↔ startup cycle, which Rust crates cannot express. Nineteen crates, one per module, would cost more in ceremony than they buy in isolation. Six layers is the middle (Jev: 0.96 for layered, 0.03 for one crate with features).

```
protocol  <-  runtime  <-  http
                  ^   <-  shell
                  |
                state   <-  engine (also uses shell)
```

Moves needed to make the layers acyclic:

- **registry ↔ startup:** registry types (module manifest, `Rule` trait) go down into `toolu-runtime`. The runner goes up into `toolu-engine`. Startup publishing stays in runtime.
- **state → detect → shell:** state only needs git facts (toplevel, branch, worktree), so read them from `.git` in `toolu-state` with no process spawn. That is also the main CPU fix from the resource diet. Shell-dependent detection (push root and branch from a `ShellAnalysis`) moves to `toolu-engine`.
- **rest → cli:** the CLI argument helpers go into runtime, and `toolu-http` depends only on runtime.

Which plugin crates pull which layers:

| Consumer | Core crates | Notes |
|----------|-------------|-------|
| jev | protocol, runtime, http | |
| statusline | protocol, runtime | |
| toolu-review | runtime, state | |
| pr-babysit | runtime, state, engine (resources, process) | |
| epic-orchestrator | runtime, state, engine, http | http for the Jira tracker |
| toolu | everything | |

`xtask check-layers` enforces the graph from `cargo metadata`:

- Core crates import only lower layers.
- Plugin crates import only core.
- Only `crates/toolu` imports rule crates.
- Only `crates/cli` builds a binary (`crates/xtask` excepted).

## Rule plugins and the registry

Today a rule plugin's SessionStart publishes an ESM bundle into `<config>/toolu/{pre,post}-tools.d/`, and the dispatcher imports every bundle on every call. In Rust (Jev: 0.95 compiled in, 0.04 one subprocess per rule):

- **First-party rules:** ts-quality, python-quality, rust-quality and the ast-grep rules are library crates implementing `toolu_runtime::Rule`. `crates/toolu` links them all.
- **Enablement:** a rule runs only when its plugin is installed. The plugin's SessionStart writes a manifest, `<config>/toolu/post-tools.d/<spec>__<name>.json`, holding spec, name, event, tool matcher and version. It is plain `sh` in `hooks.json`, with no binary needed. The dispatcher reads the directory listing and runs only rules whose matcher fits the tool. A version mismatch with the binary gives a one-line advisory.
- **Third-party modules** (today `comemory-scope.sh`, `git-better-*.sh`) keep working as executables. They take a JSON event on stdin and return a Decision on stdout, as they do now.
- **Breaking change:** third-party ESM `.js` modules are no longer loaded. That needs a `feat!` and a note in `docs/registry.md`. Until then, an optional `bun` bridge could still run `.js` modules.

## The `toolu` binary and hooks.json

One binary carries everything, so there is one artifact per platform and one name in every skill. Estimated size, stripped: 6 to 10 MB (two shell parsers, rustls, JSONC, ledger, the epic engine). A hook pays only for the pages it touches; the prototype used 2.8 MB.

```
toolu hook <event>                  # every host hook entry, called by hooks.json
toolu ledger | review | jev | babysit | epic | ast-grep | statusline | plugins …
toolu doctor | config | status | serve --stdio | commands --json | --version
```

Notes:

- **Namespaces, not argv[0]:** every plugin is a namespace of `toolu` (`toolu hook pre-tools`, `toolu epic status`). The Claude cache drops symlinks on Linux, so argv[0] aliases are out.
- **Contract:** global `--json`, data on stdout and diagnostics on stderr, documented exit codes (`2` means blocked). `toolu commands --json` exports the command tree; `docs/cli/` is generated from it, and a CI gate fails when a skill, command or agent names a verb or flag the CLI lacks.
- **Reaching the binary from Markdown:** each plugin ships a `bin/toolu` launcher, which Claude Code puts on the Bash tool's PATH. SessionStart publishes one stable path for Codex and OpenCode.
- **Launcher:** `@toolu/core/launcher` becomes `xtask print-hook`. It emits a POSIX `command` and a `commandWindows` that find the platform binary and fail closed with exit 2 on enforcing events, as the Bun launcher does now.
- **Codex trust prompt:** Codex re-prompts for trust when a hook command string changes. Keep the command identical across versions; the version lives in the binary path the launcher resolves, not in `hooks.json`.
- **Panics:** keep `panic = "unwind"` and wrap every hook `main` in `catch_unwind`. A panic must exit 2 on enforcing events and 0 with a `systemMessage` on context events, never 101. Release profile: `lto = "fat"`, `codegen-units = 1`, `strip = true`.
- **Old skill CLIs:** scripts published to stable paths today, such as `~/.claude/jev/jev.sh`, stay as one-line shims onto their namespace until the TypeScript removal.

## Workspace conventions

- **Root `Cargo.toml`:**
  - `[workspace.package] version` is the single lockstep version.
  - release-please uses the Rust/cargo-workspace strategy and keeps updating every `plugin.json` through extra-files.
  - `[workspace.dependencies]` pins serde, serde_json, brush-parser, tree-sitter(-bash), jsonc-parser, ureq, fs4, tempfile, insta, assert_cmd and proptest once.
  - `[workspace.lints]` carries the lint levels of the quality bar below.
- **zod becomes serde:** use `#[serde(deny_unknown_fields)]` plus explicit `version` handling, keeping the fail-closed envelope for `toolu.config.json` and strict v1 for the gate file. Avoid `flatten` on strict types, because `deny_unknown_fields` does not compose with it.
- **JSONC edits for OpenCode config:** `jsonc-parser` with the `cst` feature, which keeps comments.
- **Git:** read `.git` directly for toplevel, branch and worktree. Spawn `git` only for diffs and pushes.
- **Shell:** `brush-parser` first, falling back to a tree-sitter-bash partial AST on a parse error, and `bash -c` / `eval` strings re-parsed. Both are fuzzed with `cargo fuzz` in `crates/core/shell/fuzz`. Jev: 0.68 for this pairing. Unknown input still gets the conservative verdict the gates use today.

## Tests

- **Shared contract:** `fixtures/` holds the language-neutral contract. Move `tooling/fixtures/{shell,config,portable-core,gate-coverage,codex-hook-schemas}` there, and export the TypeScript golden captures (`*-golden.ts`, `*-cases.ts`, `posttool-corpus.ts`) to JSON. That export is phase 0, so both implementations read the same files.
- **During migration:** the TypeScript conformance harness spawns either the bundle or the Rust binary through one seam (`TOOLU_IMPL=rust`). About 200 black-box test files then check each Rust port for free.
- **Rust-side tests:**
  - Black-box tests in `crates/*/tests/` with `assert_cmd` and `insta` read the same `fixtures/`.
  - `proptest` covers differential parsing against the recorded unbash outputs.
  - Unit tests are co-located the way toolu's rust-quality plugin requires: no inline `#[cfg(test)]` body in `src`, a `tests/` directory beside the module, wired by a bodyless `#[cfg(test)] mod` declaration (`src/queue.rs` ↔ `src/tests/queue_test.rs`).
  - No mocks, real repos and processes as today.
- **Removing TypeScript tests:** a TypeScript test is deleted only when its plugin's Rust crate covers the same fixtures.
- **Resource budget:** `cargo xtask bench` records max RSS, CPU and wall time per hook. Its gate is the measured target, for example `toolu hook pre-tools` at 8 MB or less and 5 ms or less of CPU.

## Quality bar

The user asked for strict, well-defined gates. They exist before any product crate (epic sub-issue "Rust quality bar"), and there are no exemptions: new code has no legacy excuse. Code lines exclude blank lines and comments.

| Area | Rule | Owner |
|------|------|-------|
| File size | 300 code lines, `src` and tests | xtask guardrails |
| Function size | 50 code lines in `src` | clippy `too_many_lines` |
| `impl` block size | 200 code lines | xtask guardrails |
| Complexity | cognitive complexity 15, nesting 4, 5 parameters | clippy |
| Co-located tests | every module with a function has `tests/<module>_test.rs` beside it; no inline test bodies in `src` | xtask guardrails |
| Behaviour inventory | a passing and a failing scenario per hook, gate, rule, CLI verb and engine transition | xtask guardrails |
| Coverage | 85% of lines per crate, 90% for protocol, shell, state and engine; the floor only moves up | `cargo llvm-cov` |
| Folder structure | allowlist for the workspace, each crate and each plugin directory; no `mod.rs`, `build.rs`, `include!` | xtask guardrails |
| Architecture | layer table and capability boundaries as data (who may link HTTP, parse shell, spawn processes, read the environment, write stdout) | `xtask check-layers`, ast-grep patterns |
| Dependencies | no async runtime, no OpenSSL, no mocking crate, one version per crate, licences, advisories, nothing unused | cargo-deny, cargo-machete |
| Duplication | zero clones of 10 lines / 60 tokens or more | jscpd |
| Panics | `unsafe` forbidden; no `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, panicking index in `src` | rustc, clippy |
| Dead code | warnings denied, `unreachable_pub`, no unused public item across the workspace | rustc, `xtask check-unused-pub` |
| Suppression | no `#[allow]`, `#[expect]`, ignore list or per-path override | xtask guardrails |

How it holds:

- **One owner per rule, one source per number.** The guardrails kit already says why: "Two enforcers of one rule is how ceilings drift apart." Limits live in `toolu.config.json` and `clippy.toml`, and a test fails when two copies disagree.
- **We pass what we ship.** toolu's Rust code passes toolu's rust-quality rules at these thresholds (Jev: 0.85). The numbers are the stricter of the two house sets: 300 per file from the TypeScript tree, 50 per function and 200 per `impl` from rust-quality (Jev: 0.78 over the plugin default of 500).
- **Every rule is proven.** Each has a clean and a violating fixture, run by the gate's own tests.
- **Three places it runs:** after each edit through the rust-quality plugin, before commit and push through the quality gate, and in CI as `cargo xtask gate`.
- **Agents cannot loosen it to pass.** A limit, ban or lint level changes only in its own `chore(gates):` PR, and a PR that changes gate data together with product code fails.

All 34 lint names were checked against the installed toolchain (clippy 0.1.99, zero unknown), and jscpd was run on Rust sources. A coverage floor is new for this repository (Jev: 0.71 for adding it); the TypeScript tree has none.

## Tooling

`tooling/src` (17.6k lines) mostly becomes `cargo xtask`. Mapping:

| Today | Rust |
|-------|------|
| `build:plugins` and bundle drift | `xtask dist`: cross-build targets, write the binary manifest with sha256 |
| `check:hooks-json` | `xtask print-hook` / `check-hooks` |
| oxlint, tsc | `cargo clippy --all-targets -D warnings`, `cargo fmt --check` |
| knip | `cargo machete` |
| jscpd | keep jscpd (it reads Rust) |
| gate-reach, legacy exemptions | mostly retired, since cargo covers every workspace member; keep a small `xtask check-layers` |
| guardrails | port the runner; the data in `tooling/conventions/guardrails/` stays |
| context budget | `xtask context-budget` (it reads Markdown) |
| benchmarks | `xtask bench` |

OpenCode acceptance (`tooling/src/opencode-*`) drives the real Bun-hosted OpenCode, so it stays TypeScript next to `packages/opencode-shim` until the end.

## OpenCode and the npm CLI

- **OpenCode:** `packages/opencode-shim` (about 300 lines of TypeScript, down from 9k) starts one long-lived `toolu serve --stdio` per OpenCode instance and exchanges JSON lines per event, restarting it if it dies. Jev: 0.51 for this, 0.42 for an in-process napi addon. Choose napi only if the round-trip cost shows up in acceptance budgets.
- **npm CLI:** `@toolu/plugins` becomes a small npm wrapper over per-platform `optionalDependencies` packages, the biome/esbuild pattern. The same platform packages give the OpenCode shim its binary, because OpenCode installs npm plugins with Bun. `@toolu/core` stops being published at the end, after a deprecation release.

## Removed plugins

exa-search, context7, jira and agent-browser are deprecated and removed. Together they are about 2.6k lines of TypeScript, and they appear in 41 to 83 files each outside their own directories. Most of those files are OpenCode generated files and the capability matrix, OpenCode acceptance and host tooling, toolu-cli install lists, release-please config, the marketplace and docs.

**Deprecation path** (Jev: 0.99 for notice-then-remove over remove-at-once):

1. **Notice release (TypeScript, before phase 0):**
   - Each of the four plugins' SessionStart prints one `systemMessage` line: deprecated, removed in the next minor, plus the uninstall command for the host.
   - Each README gets a deprecated banner.
   - Nothing else changes, so users have one release to uninstall.
2. **Removal release (`feat!`, the first PR of the Rust program, before phase 1):** delete the four plugin trees, `docs/<plugin>`, their entries in `.claude-plugin/marketplace.json`, `release-please-config.json`, toolu-cli install lists, OpenCode generated files, the capability matrix (16 to 12 plugins), contract probes and acceptance scenarios, and the gate-coverage inventory.

**Timing:** Jev leaned toward removing in phase 5, where the plugins would have been ported (0.57 against 0.32 for before phase 0). I recommend removing before phase 1 instead. Phases 1 to 4 change `startup`, `rest` and the OpenCode matrix, and the phase 0 fixture export would otherwise include fixtures for plugins that are going away. Keeping them alive that long only creates work.

**Dependents to rewire in the same removal PR:**

- **epic-orchestrator Jira tracker:** stays, with its own client (user decision).
  - Today `scripts/trackers/jira.ts` (234 lines) runs `jira.sh` for JQL search of children, issue get, transition and comment, and worker briefs tell agents to run `jira.sh issue get <KEY>`.
  - Replacement: a small REST client inside epic-orchestrator, in TypeScript now and in `crates/epic-orchestrator` later, plus a `jira issue get <KEY>` subcommand that briefs call instead.
  - Auth from env only: `JIRA_BASE_URL` with `JIRA_EMAIL` + `JIRA_API_TOKEN` (basic) or `JIRA_PAT` (bearer), and `JIRA_API_VERSION` 2 or 3.
  - The jira-cli config file and keyring fallbacks are dropped, a narrowing to note in the CHANGELOG.
  - `EPIC_JIRA_SH` goes away.
  - Its tests move off the jira bundle onto the loopback HTTPS fixture.
- **Research routing:** the research-agent, the deep-research skill, `lifecycle/tool-mandates.ts` and the research line in `lifecycle/prompt-hints.ts` stop naming exa-search and context7 and use the host's native web search and fetch. The Jira prompt hint is deleted.
- **Core:** after the removal, `@toolu/core/rest` serves jev and epic-orchestrator only, so `toolu-http` stays small. Removing exa-search and context7 drops the `EXA_API_KEY` and `CONTEXT7_API_KEY` handling in the conformance env patches.

## Migration order

Each phase merges green and ships. TypeScript and Rust hooks coexist, and the launcher picks per entry. This is the order as first sketched; epic #402 numbers its phases −1 to 7 and is authoritative.

| Phase | Scope | TS lines replaced | Parity oracle |
|-------|-------|-------------------|---------------|
| −1 | Deprecation notice release, then the removal release with epic-orchestrator's built-in Jira client and research rerouting (see "Removed plugins") | about 2.6k removed | existing tests minus the removed plugins; Jira tracker tests on the loopback fixture |
| 0 | Workspace skeleton, `fixtures/` JSON export, `TOOLU_IMPL` harness seam, `xtask bench` baseline, distribution decision | — | — |
| 1 | `protocol`, `runtime`, `state` | about 4.5k | config and state fixtures; the gate file stays byte-identical, because both runtimes write it during coexistence |
| 2 | `shell` plus fuzzing | 1.3k | `bats-parity.json`, `issue-283.json`, recorded unbash outputs |
| 3 | `engine` and `crates/toolu` dispatchers, gates, ledger | about 7k | golden captures, lifecycle, gate coverage |
| 4 | Rule crates compiled in, manifest registration | about 2.5k | post-tool corpus, quality tests |
| 5 | Leaf namespaces: jev, statusline, toolu-review | about 1.5k | HTTPS loopback fixture via `HTTPS_PROXY` (`ureq` honours it) |
| 6 | pr-babysit and the epic engine (see the epic-engine brainstorm) | about 10.5k | herdr smoke items, real gh/git sandboxes, loopback Jira fixture, a scripted epic |
| 7 | `toolu plugins`, `opencode-shim`, npm platform packages | about 7k (plus 9k adapter) | `test:opencode` acceptance on Linux and macOS |
| 8 | xtask replaces `tooling/src`; delete TS and the Bun prerequisite for Claude Code and Codex | about 17k | the whole gate |

Phase 3 is the first one users feel (pre/post-tool hooks), so the distribution decision has to land before it ships.

## What I think

- **The layout is right with two amendments.**
  - One crate per plugin maps cleanly onto how the plugins are installed and owned. But the crates are libraries behind one `toolu` binary, and rule plugins are linked into the dispatcher: spawning one process per rule would bring back the per-call cost this rewrite removes.
  - `crates/core/` should be a directory of six crates, not one crate. Otherwise every plugin crate depends on the shell parser and the TLS stack, and the registry ↔ startup cycle has nowhere to go.
- **Rust suits this workload.** Hooks are short-lived, JSON in and JSON out, with strict schemas and fail-closed exits. serde's strictness replaces zod for free, and the 2.8 MB / under-10 ms prototype shows the headroom.
- **Main costs:**
  - About 45k lines of source and a similar test volume, a week after the bash → TypeScript migration.
  - Binary distribution, which TypeScript never needed.
  - Shell-parser fidelity, which becomes a security question because the gates depend on it.
  - The strangler order and the existing black-box suite make the source and test volume manageable. The other two need their own decisions.
- **Breaking changes to announce:**
  - exa-search, context7, jira and agent-browser are removed after one release of notice. epic-orchestrator keeps Jira epics through its own client, with env credentials only.
  - Third-party ESM registry modules go away; executables remain.
  - `@toolu/core` is deprecated on npm.
  - Bun stops being a prerequisite on Claude Code and Codex, while OpenCode keeps it because OpenCode itself runs on Bun.

## Open decision

How Claude Code and Codex installs get the binary. Jev split 0.52 / 0.47 between:

- **Download at SessionStart:** the binary for the platform comes from the GitHub Release, is checked against a pinned sha256, and is installed into `CLAUDE_PLUGIN_DATA` / `PLUGIN_DATA`. The repo stays tiny. Needs network on first run. Enforcing hooks fail closed until the binary is present.
- **Separate dist repo:** release CI publishes a marketplace repo, squashed to one commit per release, whose plugin dirs carry prebuilt binaries for every platform. Works offline, with no download code. Users re-add the marketplace once.

Committing binaries into this repo was rejected because every release would add tens of MB to git history. npm per-platform packages are kept for `npx @toolu/plugins` and OpenCode only.
