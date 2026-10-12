# Epic engine actions — graph, route, launch, finish, close, trackers

Brainstorm, 2026-10-11. Delivery mode for Falconiere/toolu#435. The
2026-10-04 engine brainstorm stays the product decision and the 2026-10-07
engine-core note stays the engine contract; this note records the choices #435
has to make to replace `plugins/epic-orchestrator/scripts` with in-process Rust
actions while #433 and #448 are still open.

## Capsule

- **Outcome:** `toolu epic graph|route|launch|finish|close|release|gate|jira|probe`
  are real verbs, and the resident engine runs graph refresh, routing, launch,
  recovery, finish and close itself, for GitHub, Jira and Linear epics. The
  TypeScript `scripts/` and `hooks/src` trees are gone; the plugin's Markdown
  runs only `toolu epic …`.
- **Material defaults:**
  - Effects run in process on one effect thread; the state thread never blocks
    on herdr, git or a tracker. Launch, cleanup, reconcile and prompt stop
    using `TOOLU_EPIC_SCRIPTS`; only `merge` (and its reconcile) keep the
    script seam until #448.
  - The merge gate is ported verdict for verdict as the manual `toolu epic
    gate` verb. The engine still does not merge unattended: a ready issue
    raises a `merge-ready` judgment so the orchestrator runs the gate (Jev
    0.80 over deferring entirely, 0.20). #448 adds the queue and calls the gate
    itself.
  - `check-deps.ts`, `check-binary.ts` and `engine-ensure.ts` become one native
    `toolu epic hook session-start` (Jev 0.56 for porting check-deps).
    `probe.ts` becomes `toolu epic probe` (Jev 0.62).
  - Trackers: GitHub through `toolu-github`, Jira and Linear through
    `toolu-http`, with today's retry policy (`EPIC_API_ATTEMPTS`,
    `EPIC_API_BACKOFF_MS`, `EPIC_API_MAX_SLEEP_S` capped at 15 minutes,
    `Retry-After`, no retry of a mutation that may have applied).
  - Jev routing and the coupling check use `toolu-jev-client` in process. A
    coupling probability of 0.80 or more holds the lower-priority issue (Jev
    split 0.44 for 0.75 and 0.49 for 0.90; 0.80 sits between).
  - Tests: Rust unit and black-box tests replace the script suites, with an
    in-process herdr socket fixture (Jev 0.98), the HTTPS fixture for GitHub,
    Jira and Linear, and real git. The two TypeScript tests outside the plugin
    that imported scripts go through the seam or committed JSON (Jev 0.95).
  - Markdown: a command swap that keeps the skill's structure; #452 owns the
    redesign (Jev 0.85).
- **Non-goal:** the merge queue, mechanical rebase and unattended merge (#448),
  the babysit tick (#433), the skill redesign and evals (#452), and removing
  Bun from other plugins (#440).
- **Repository evidence:** the issue and its acceptance list; #448's scope
  (gate verdicts, `toolu epic gate`); #434's note ("unattended merge in
  production stays deferred until #448", "#435 and #448 replace the script
  directory"); the crate's `planned` list; the existing `World`, `Step`,
  `commit_effect`, `herdr::Client`, `checkpoint::snapshot`,
  `toolu_engine::resources`, `toolu_runtime::startup::{native_toolu_advice,
  codex_dependency_notice}`; the TypeScript scripts and their tests.
- **Risk:** the port is large (about 6,800 lines of TypeScript, 4,200 of
  tests). The engine's effect execution changes from synchronous scripts to an
  effect thread. Recovery of an in-flight launch after a crash must not
  start a second agent.
- **Handoff:** spec.

## Axes

- **Intent.** Every deterministic action the orchestrator LLM ran by hand is an
  engine transition and a CLI verb. Judgments stay judgments: `needs-human`,
  `failed`, `blocked`, stall after `STATUS?`, `dependency-cycle`,
  `coupling-hold`, `cleanup-incomplete`, `relaunch-limit`, plus the bridging
  `merge-ready` until #448.
- **Data.** The per-epic JSON files keep their TypeScript shapes: `graph.json`,
  `routes/<key>.json`, `pool.json`, `hosts.json`, `issues/<key>.json` (launch
  record, removal intent), `status/<key>.json`, `briefs/<key>.md`. Holds live
  in the issue record (`hold`), so a restart keeps them. The journal records
  every launch, hold, release, reroute and close.
- **Interface.** New verbs and flags only; no documented command or flag is
  removed, so `HOOK_PROTOCOL` stays. `toolu epic report` keeps its #434 shape;
  the brief renders it.
- **Failure.** A launch outcome that cannot be verified keeps the issue
  occupied (`uncertain`), as today. Capacity refusal is not a failure: the
  issue waits for the next slot. Cleanup refuses over work it could not
  snapshot and while a worktree-owned process group lives. Admission refuses a
  replacement while the old workload remains.
- **Integration.** Plugin crates link only core: trackers use `toolu-http` and
  `toolu-github`, Jev uses `toolu-jev-client`, processes go through
  `toolu_runtime::process`.
- **Horizon.** #448 replaces the `merge-ready` judgment with the queue and
  removes the last script seam.

## Alternatives

| Decision | Chosen | Rejected and why |
|---|---|---|
| Merge gate | Manual `toolu epic gate`, engine merge deferred with a `merge-ready` judgment | Defer entirely (0.20): the skill would lose its gate when `scripts/` goes. Keep `merge-gate.ts` (0.00): misses the deletion acceptance. Engine merges unattended (0.00): contradicts #434's agreed deferral to #448. |
| Effect execution | One effect thread posting outcomes back to the state loop | Synchronous on the state thread: a launch takes up to three minutes (clone, `agent start` 90 s, prompt wait 60 s, session capture 10 s), and a batch of three would block reports and `wait` for nine. One thread per effect: today's launches are deliberately one at a time. |
| herdr in tests | In-process socket fixture | A fake `herdr` on `PATH` (0.00): the engine speaks the socket; real herdr only (0.02): CI has none. |
| Script tests | Rust tests, and the seam for the two external TypeScript tests | TypeScript native suites (0.00), deleting the OpenCode live worker test (0.05). |
| Markdown | Command swap | Full redesign now (0.12) belongs to #452; deleting lines only (0.03) leaves the skill broken. |
| check-deps, probe | Ported | Removal: both are small, and the Rust runtime already has the dependency notice. |
