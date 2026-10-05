# Hook resource diet (instead of a Rust rewrite, for now)

> Superseded the same day: the user chose to rebuild the plugins in Rust (epic #402, [2026-10-04-rust-workspace.md](2026-10-04-rust-workspace.md)). The TypeScript diet below was not built. The measurements and the CPU profile here are the evidence that rebuild rests on, and the git-spawn and registry-import fixes carry over to the Rust design.

Brainstorm, 2026-10-04. Request: "refactor toolu plugins to Rust". Stated motivation: hook memory and CPU usage.

## Capsule

- **Outcome:** each hook spawn costs measurably less memory and CPU, and CI keeps it that way. Targets: `pre-tools.js` ≤ 22 MB RSS and ≤ 15 ms CPU (from 38 MB, ~45 ms); `post-tools.js` with the usual registry ≤ 26 MB and ≤ 25 ms (from 46 MB, 90 to 150 ms). These numbers are the bar for the spec, not promises.
- **Material defaults/non-goal:** stay on TypeScript and Bun. Fix what the profile shows, and add a resource budget gate. No Rust in this round. OpenCode is unchanged: it runs toolu in process and has no per-call spawn cost.
- **Repository evidence:** the 2026-09-26 runtime decision (epic #247) chose committed Bun bundles with no compiled binaries, because OpenCode needs TypeScript in process. Measurements and a CPU profile from this session are below.
- **Risk:** Bun's 13 MB floor stays. If the diet stops at about 20 MB per spawn and parallel agents still hurt, the next step is a Rust dispatcher (see "Decision gate").
- **Handoff:** `/delivery-flow:delivery-flow` with this file as input.

## Evidence

Measured on the user's Mac with warm runs and `/usr/bin/time -l`. CPU time includes reaped children.

| Process | Max RSS | CPU |
|---------|---------|-----|
| Rust trivial binary (stdin read, `{}` out) | 1 MB | ~0 ms |
| `bun` on an empty file | 13 MB | ~0 ms |
| `pre-tools.js`, Bash `git status` payload | 38 MB | 45 to 49 ms |
| `post-tools.js`, Read payload, installed registry | 46 MB | 90 to 150 ms |
| `post-tools.js`, empty registry | 31 MB | ~30 ms |
| `pre-tools.js` re-minified | 41 MB | 49 ms |

- Minifying does nothing, so parse time is not the cost. `bun build --bytecode` and `--compile` are blocked by top-level await in the entries.
- CPU profile of `pre-tools.js`: about half the samples are `spawnSync` of `git rev-parse --show-toplevel`, called twice per hook, from `loadConfig → configFiles → projectConfigPath` and from `sessionFor` in `dispatchHook`. Most of the rest is importing registry modules.
- Registry modules (`<config>/toolu/{pre,post}-tools.d/*.js`) are full bundles, each inlining its own copy of `@toolu/core` and zod. The dispatcher imports all of them before checking whether any applies to the tool. They add about 15 MB and about 60 ms to every `post-tools.js` run.
- Long-lived toolu processes: two `epic-watch.ts` at 31 MB each. The largest consumers on the machine were unrelated (vitest 743 MB, a Claude session 425 MB).
- Fan-out: each tool call runs toolu's pre and post dispatchers plus other plugins' hooks. With many parallel herdr agents, about 40 MB and 50 to 150 ms per spawn adds up.

## Axes

- **Interface/compatibility:** the registry module contract (`defineRegistryModule`, ESM default export) stays. A per-module manifest written at register time (event, tool matcher) is additive: modules without one are still imported, as they are today.
- **Persistence/migration:** none. Registry files are re-published at SessionStart.
- **Cost:** a few days of targeted TypeScript work, against months for a 45k-line rewrite plus a second gate implementation for OpenCode.
- **Reversibility:** high. Each fix stands alone, and the budget gate makes later regressions visible.

## Approach

1. **No git spawn for the toplevel.** Find the toplevel by walking up from `cwd` for `.git` (a directory, or a file for worktrees), and resolve it once per process. Today it is spawned twice per hook through `gitToplevel` (in `host-roots.ts` and `dispatch.ts`, also used by gates and detect). Keep `git` only where its exact semantics matter, with golden fixtures for worktrees and submodules.
2. **Lazy registry imports.** `register.ts` writes a sidecar manifest next to each module (event and tool matcher). The dispatcher imports only modules whose matcher fits the current tool. A Read call then imports no quality module.
3. **Thinner hot path.** Move zod validation off the per-call path where the input is already trusted, or validate lazily, and check whether `post-tools` needs the full shell parser on every event. Measure each change and keep only what the numbers support.
4. **Resource budget gate.** Extend `bench:final-hooks` (`tooling/src/benchmarks/final-hook-latency.ts`, `post-tools-latency.ts`) so it records max RSS and CPU time per dispatcher with a fixed registry fixture, and fails above the committed budget.
5. **Watcher footprint.** Check whether `epic-watch.ts` can idle smaller, for example by polling from a short-lived process instead of staying resident. Spec decides.

## Alternatives

| Option | Verdict | Why |
|--------|---------|-----|
| Full Rust rewrite (core, plugins, CLI, OpenCode adapter) | Rejected | About 45k lines of source and 45k of tests, rewritten one week after the bash-to-TypeScript migration. OpenCode still needs TypeScript in process, so gates would exist twice. Jev: 0.00. |
| Rust native dispatcher, shell analysis and quality rules | Deferred | The best per-spawn floor (1 MB vs 13 MB). It breaks the ESM registry contract, needs per-platform binaries through a git marketplace (committed or downloaded at session start), and duplicates gate logic with the OpenCode TypeScript path. Worth it only if the diet misses its budget. |
| Rust core compiled to WASM, loaded by Bun | Rejected | It still pays the Bun floor and adds WASM instantiation. No memory win. |
| Leave as is | Rejected | The profile shows cheap, specific waste. |
| Targeted TypeScript diet plus budget gate | **Chosen** | It goes after the measured causes (git spawns, eager registry imports) at low cost and stays reversible. Jev: 1.00. |

## Decision gate for Rust

Reopen the Rust dispatcher only if, after steps 1 to 4, the remaining per-spawn cost is mostly the Bun floor and the user still sees pressure from parallel agents. Jev put this at 0.44: open, not likely. The spec for that would start from the golden captures (`tooling/fixtures/shell`, gate goldens) as the parity contract and solve binary distribution first.
