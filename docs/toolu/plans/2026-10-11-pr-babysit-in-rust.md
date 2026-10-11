# pr-babysit in Rust — Plan

**Date:** 2026-10-11   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-11-pr-babysit-in-rust-design.md   **Topic:** `toolu babysit` replaces the Bun pr-babysit bundles

## Evidence and approach

Inspected: the TS sources under `plugins/pr-babysit/hooks/src` (tick, collect,
reduce, verdict, writes, record, route, dispatch, process), their suites and
goldens; `toolu-github` (`Client`, `Config::one_shot/scheduled`, `Error`),
`toolu-jev-client`, `toolu_runtime::{process, startup, atomic, cli}`,
`toolu_state::time`; the engine's `BabysitTick` link
(`crates/core/engine/src/babysit.rs`, `crates/epic-orchestrator/src/{babysit,
github_engine, schedule, server, source}.rs`, `watch::CHECK_MS = 180_000`);
precedent ports #504 (jev) and #510 (statusline). Recall: memory 82d578fe
(two-space Rust, installed binary).

Approach as in the spec: port module by module with the goldens moved to
`fixtures/pr-babysit/`, then the engine link, then hooks and Markdown, then
delete the Bun sources. Each step's tests run against real fixtures (recorded
GitHub replies over loopback TLS, real git repositories, a real herdr server
for the smoke).

## Workstream summary

foundation → verdict → reduce → collect → tick → writes/record → wait →
route-fix → runtime process helpers → dispatch-fix → hooks and CLI wiring →
engine prompt → Markdown, OpenCode generator, tooling, deletion → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "foundation",
    "title": "toolu-pr-babysit foundation: error.rs (closed codes, exit mapping 64/69/75/1, error document), sha1.rs (FIPS 180 vectors), lock.rs (SlotLock mkdir protocol with pid/since, stale rules, PB_LOCK_STALE_SECONDS), state.rs (load v2 with repo regex, slot_mismatch, atomic JSON write), fixtures moved to fixtures/pr-babysit/",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": ["AC-1"],
    "paths": ["crates/pr-babysit/", "fixtures/pr-babysit/", "Cargo.toml", "Cargo.lock"],
    "input": "real state files fixtures/pr-babysit/states/toolu-165-initial.json; a lock dir whose pid is dead; a lock dir younger than 1 s without pid; abc and the 448-bit FIPS SHA-1 vectors",
    "model": "inherit"
  },
  {
    "id": "verdict",
    "title": "verdict.rs ports verdict.ts (markers, checklist, label, provider_error, findings path:line:sha1[0..8], Top-N must-fix) and the parse-verdict verb reading stdin",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib verdict 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1"],
    "depends_on": ["foundation"],
    "paths": ["crates/pr-babysit/src/verdict.rs", "crates/pr-babysit/src/tests/verdict_test.rs", "fixtures/pr-babysit/parse-verdict.golden.json", "fixtures/pr-babysit/verdicts/"],
    "input": "fixtures/pr-babysit/parse-verdict.golden.json and the captured bot comment texts",
    "model": "inherit"
  },
  {
    "id": "reduce",
    "title": "reduce/ ports reduce.ts without backoff; adds attention {needed,new,codes,fingerprint} and schedule fields; reduce-state verb with the shipped flags",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib reduce 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["verdict"],
    "paths": ["crates/pr-babysit/src/reduce.rs", "crates/pr-babysit/src/reduce/", "crates/pr-babysit/src/tests/", "fixtures/pr-babysit/reduce-state.golden.json", "fixtures/pr-babysit/snapshots/"],
    "input": "fixtures/pr-babysit/reduce-state.golden.json cases over snapshots toolu-115, toolu-165, comemory-216",
    "model": "inherit"
  },
  {
    "id": "collect",
    "title": "collect/ reads the PR through toolu-github (GraphQL PR + rollup + threads + thread comments, paginated REST comments and reviews), head re-check, snapshot v1; collect verb; api_error/gh_unavailable mapping",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib collect 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1"],
    "depends_on": ["foundation"],
    "paths": ["crates/pr-babysit/src/collect.rs", "crates/pr-babysit/src/collect/", "crates/pr-babysit/src/tests/", "fixtures/pr-babysit/collect-pr-115-p2.golden.json", "fixtures/pr-babysit/github/"],
    "input": "recorded PR 115 pages served by toolu-http-test-support at page size 2; a head that moves twice; GraphQL errors[]; 502 then 200; no token",
    "model": "inherit"
  },
  {
    "id": "tick",
    "title": "tick.rs: lock, collect or --snapshot-in, reduce, atomic snapshot/state/result writes, lastError stamping; Tick implements BabysitTick (Success, Act with prompt text, KeepGoing); toolu-engine TickDecision::Act and TickReport.prompt",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib tick 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-engine --lib babysit 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["reduce", "collect"],
    "paths": ["crates/pr-babysit/src/tick.rs", "crates/pr-babysit/src/tests/tick_test.rs", "crates/core/engine/src/babysit.rs", "crates/core/engine/src/tests/"],
    "input": "snapshot toolu-165 replay; a held lock; a malformed state; a fixture PR with one CI-reviewer thread ticked twice",
    "model": "inherit"
  },
  {
    "id": "writes",
    "title": "reply, resolve, record (flag-injection, round, status) and stop verbs over toolu-github with the slot lock",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib writes 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib record 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1"],
    "depends_on": ["tick"],
    "paths": ["crates/pr-babysit/src/writes.rs", "crates/pr-babysit/src/record.rs", "crates/pr-babysit/src/tests/"],
    "input": "loopback replies for the REST reply endpoints and resolveReviewThread (true, and three false); a duplicate in-reply-to; an empty body file; a 70 KB body",
    "model": "inherit"
  },
  {
    "id": "wait",
    "title": "wait.rs: anchored 180 s schedule, Clock and Check seams, per-host --max-seconds default, waiting document; virtual-clock tests for AC-4, AC-5 and the pending half of AC-6; binary test with --max-seconds 1",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib wait 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-4", "AC-5", "AC-6"],
    "depends_on": ["tick"],
    "paths": ["crates/pr-babysit/src/wait.rs", "crates/pr-babysit/src/tests/wait_test.rs"],
    "input": "loopback PR fixture pending, then approved and green at t; virtual clocks for claude, codex, opencode; a 6 h pending run",
    "model": "inherit"
  },
  {
    "id": "route",
    "title": "route/ ports fixer-route.ts: prBabysit config per host, cooldowns, PATH hosts, heuristic and Jev scoring through toolu-jev-client in process, --jev-answers-in replay, --raise, groups",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib route 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1"],
    "depends_on": ["foundation"],
    "paths": ["crates/pr-babysit/src/route.rs", "crates/pr-babysit/src/route/", "crates/pr-babysit/src/tests/", "fixtures/pr-babysit/jev/", "fixtures/pr-babysit/items/"],
    "input": "fixtures/pr-babysit/items/review-items.json and jev/fix-tiers.json; a loopback Jev origin; temp user and project toolu.config.json; a PATH without opencode",
    "model": "inherit"
  },
  {
    "id": "process",
    "title": "toolu-runtime process: spawn_logged (own group, null stdin, output appended to a log, returns pid) and group::signal_pid",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib process 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-8"],
    "paths": ["crates/core/runtime/src/process.rs", "crates/core/runtime/src/process/", "crates/core/runtime/src/tests/"],
    "input": "sh -c 'echo out; echo err >&2; sleep 30' spawned, logged and signalled",
    "model": "inherit"
  },
  {
    "id": "dispatch",
    "title": "dispatch/ ports fixer-dispatch.ts and fixer-process.ts (start, wait, cleanup, brief from include_str! fixer-brief.md, herdr calls, native worktree, OpenCode detached fixer, host_limited, trust prompt) and the fixer-report verb",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib dispatch 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": ["AC-1", "AC-8"],
    "depends_on": ["route", "process", "tick"],
    "paths": ["crates/pr-babysit/src/dispatch.rs", "crates/pr-babysit/src/dispatch/", "crates/pr-babysit/src/tests/", "plugins/pr-babysit/skills/babysit/references/fixer-brief.md", "fixtures/pr-babysit/opencode/", "fixtures/pr-babysit/herdr/"],
    "input": "real git repos with a bare origin; an opencode stand-in executable on PATH that writes JSON events and sleeps; fixtures/pr-babysit/opencode/fixer-r1g1.log; herdr/claude-trust-prompt.txt",
    "model": "inherit"
  },
  {
    "id": "hooks-cli",
    "title": "check-toolu and check-binary native hooks, crates/cli registry/dispatch/hook wiring, hooks.json from cargo xtask print-hook, docs/cli regenerated, commands snapshot accepted",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-pr-babysit --lib hooks 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-cli && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only hooks --only docs-cli --only cli-compat",
    "ac_refs": ["AC-1", "AC-10"],
    "depends_on": ["dispatch", "wait", "writes"],
    "paths": ["crates/pr-babysit/", "crates/cli/", "plugins/pr-babysit/hooks/hooks.json", "plugins/pr-babysit/.claude-plugin/plugin.json", "docs/cli/"],
    "input": "temp Codex home without the toolu core plugin; a PATH with a native toolu and one without",
    "model": "inherit"
  },
  {
    "id": "engine",
    "title": "epic-orchestrator: Next::Prompt from TickDecision::Act, journal prompt/babysit, Step::Prompt {key,text} shared with the STATUS? nudge; scripted epic tests for AC-2, AC-3 and the red-CI half of AC-6 with toolu-pr-babysit as a dev-dependency",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-epic-orchestrator 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && ! printf '%s' \"$o\" | grep -q FAILED",
    "ac_refs": ["AC-2", "AC-3", "AC-6"],
    "depends_on": ["tick"],
    "paths": ["crates/epic-orchestrator/", "crates/core/engine/src/babysit.rs", "crates/pr-babysit/src/tick.rs", "crates/cli/src/links.rs"],
    "input": "loopback GitHub serving a green approved PR; one with a single CI-reviewer thread switched to approved; one with red CI for 6 virtual hours",
    "model": "inherit"
  },
  {
    "id": "docs-delete",
    "title": "Rewrite plugins/pr-babysit Markdown to toolu babysit (no cron, no backoff, wait loop), OpenCode generator and acceptance scenarios, herdr smoke, tooling references, AGENTS.md, docs/pr-babysit; delete plugins/pr-babysit/hooks/src and hooks/dist",
    "check": "test ! -e plugins/pr-babysit/hooks/src && test ! -e plugins/pr-babysit/hooks/dist && ! rg -n 'hooks/dist|scripts/|\\bbun\\b|CronCreate' plugins/pr-babysit && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-markdown-cli && bun test tools/toolu-opencode tooling/src/__tests__/pr-babysit-herdr-smoke.test.ts",
    "ac_refs": ["AC-9", "AC-10"],
    "depends_on": ["hooks-cli", "engine"],
    "paths": ["plugins/pr-babysit/", "tools/toolu-opencode/", "tooling/", "AGENTS.md", "docs/"],
    "input": "the repository tree and the generated OpenCode surfaces",
    "model": "inherit"
  },
  {
    "id": "live-tick",
    "title": "Branch build ticks a real GitHub PR (Falconiere/toolu#510, merged) through toolu-github: decision escalate with pr_merged, snapshot and result files written",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo build -q -p toolu-cli && d=$(mktemp -d) && o=$(target/debug/toolu babysit tick --repo Falconiere/toolu --pr 510 --state-file \"$d/s.json\") && printf '%s' \"$o\" | grep -q '\"pr_merged\"' && test -s \"$d/s.snapshot.json\" && test -s \"$d/s.result.json\"",
    "ac_refs": ["AC-7"],
    "depends_on": ["hooks-cli"],
    "paths": ["crates/pr-babysit/", "crates/cli/"],
    "input": "live GitHub PR Falconiere/toolu#510 with the user's gh token",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full gate and the herdr smoke against the branch build",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main --title 'feat(pr-babysit): port pr-babysit to toolu babysit'",
    "ac_refs": ["AC-1", "AC-10"],
    "depends_on": ["docs-delete"],
    "paths": ["."],
    "input": "the whole repository",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/pr-babysit/src/**` (new modules and `tests/`), `crates/pr-babysit/Cargo.toml`
- `crates/core/engine/src/babysit.rs`, `crates/core/runtime/src/process*`
- `crates/epic-orchestrator/src/{babysit,github_engine,model,schedule,server,source}.rs` and tests
- `crates/cli/src/{registry,dispatch,links}.rs`, `crates/cli/src/hook/plugins.rs`
- `fixtures/pr-babysit/**`, `plugins/pr-babysit/**`, `docs/cli/**`
- `tools/toolu-opencode/scripts/lib/opencode-port-pr-babysit.ts`, `tooling/src/opencode-host/{babysit-fixture,scenarios-babysit}.ts`, `tooling/src/pr-babysit-herdr-smoke.ts`
- `AGENTS.md`, `docs/pr-babysit/README.md`, `tooling/conventions/guardrails/rust/{folders,inventory,coverage-floor}.json` as registration requires

## Verification

`cargo xtask gate --base origin/main` green; `cargo test -p toolu-pr-babysit`
and `-p toolu-epic-orchestrator` cover AC-1…AC-6 and AC-8 with real fixtures;
the herdr smoke and a live `toolu babysit wait` on this PR (AC-7) run against
the branch build; `cargo xtask check-markdown-cli` and the AC-9 tree checks
pass; docs listed in the spec's Documentation impact are updated in the
`docs-delete` step.

Delivery: commit after each green step (`wip:` allowed) and push the branch;
before the PR, rebase on `origin/main`, run the gate through the epic job
wrapper, `plan-ledger.js run <plan> --verify`, `toolu-review:review` and
`verdict.js status` (`overall: ready`). Then open the PR, and the babysit
phase runs `toolu babysit wait` from the branch build on it (AC-7's clearing
evidence goes in the PR body). Prerequisites: authenticated `gh api user`,
a non-default branch, installed brainstorm, toolu, toolu-review and
pr-babysit skills.

## Plan review (2026-10-11)

- Steps: 🟡 should-fix: AC-7 had no ledger step. Fixed: `live-tick` on a real merged PR; clearing proven on this PR in babysit.
- Verification: 🟡 should-fix: delivery sequence missing. Fixed above.
- Paths: 🔵 consider: TS sources are not declared because the `docs-delete` step removes them; the Rust tests carry the goldens.

**Status:** Approved
