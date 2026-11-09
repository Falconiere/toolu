# Epic: Grok as a worker host — Plan

**Date:** 2026-11-08   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-08-epic-grok-worker-host-design.md   **Topic:** `toolu epic` runs Grok Build workers

## Evidence and approach

The work mirrors the Muse worker host (PR #607, plan
`docs/toolu/plans/2026-10-23-epic-muse-worker-host.md`) inside
`crates/epic-orchestrator`. Adding `HostKind::Grok` lets the compiler flag
every exhaustive `match` (`hosts.rs`, `launch/session.rs`, `launch/screen.rs`,
`tmux_backend.rs`). Code inspected:

- `HOST_KINDS` already drives `status_resources.rs` and `route/pool.rs`.
- `limit.rs` and `probe_evidence.rs` keep literal host lists.
- `gate/local.rs:98` and `status_plan.rs:104` look up the ledger with
  `Host::parse(kind)`, which fails for `grok`.

Every Grok fact comes from the 2026-11-08 live probe on Grok Build 1.0.77 and
herdr 0.9.5 (spec "Live facts"). Tests use the launch rig
(`src/tests/launch_rig_test.rs`): a real shell agent, a real checkout and the
herdr socket fixture. Session and token tests use trimmed real Grok files.
Comemory recall was unavailable (embedding 404).

## Workstream summary

host model (argv, limits, ledger host) → routing → session capture → tokens →
exit key → status/limit/evidence lists → ledger lookups → live rig launch →
CLI help, docs-cli and contract → user docs → gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "HostKind::Grok: argv, skill_ref, error text, HOST_LIMIT Grok lines, ledger_host; exhaustive arms; fixture cases",
    "check": "cargo test -p toolu-epic-orchestrator --lib hosts::tests",
    "ac_refs": ["AC-2", "AC-7", "AC-8"],
    "paths": ["crates/epic-orchestrator/src/hosts.rs", "crates/epic-orchestrator/src/tests/hosts_test.rs", "crates/epic-orchestrator/src/launch/screen.rs", "crates/epic-orchestrator/src/tmux_backend.rs", "fixtures/epic/hosts.json", "fixtures/index.json"],
    "input": "the probed argv (herdr agent start --kind grok: --always-approve, --permission-mode default, --model/--reasoning-effort, --resume <uuid>, --continue); Grok 1.0.77 throttle strings ('You've hit the credit limit for your plan.', 'subscription:free-usage-exhausted', the rate-limit line); boundaries: 'credit card form validated' does not match, a model with a space is refused, opencode/gemini stay unknown with grok listed, ledger_host maps grok to Claude and leaves the other four unchanged"
  },
  {
    "id": "s2",
    "title": "Grok routing defaults per tier and routing.json override",
    "check": "cargo test -p toolu-epic-orchestrator --lib route::table",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1"],
    "paths": ["crates/epic-orchestrator/src/route/table.rs", "crates/epic-orchestrator/src/route/tests/table_test.rs"],
    "input": "default table (grok-4.7-build-fast/low, grok-4.7/medium, grok-4.7/high, grok-4.7/xhigh, all accepted live) and a temp routing.json with hosts.grok; boundary: a routing.json without grok keeps the Grok defaults"
  },
  {
    "id": "s3",
    "title": "Grok session capture from the real <GROK_HOME>/sessions/<encoded cwd>/<uuid>/summary.json layout",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::grok_session",
    "depends_on": ["s1"],
    "ac_refs": ["AC-3"],
    "paths": ["crates/epic-orchestrator/src/launch/grok_session.rs", "crates/epic-orchestrator/src/launch/tests/grok_session_test.rs", "crates/epic-orchestrator/src/launch/session.rs", "crates/epic-orchestrator/src/launch.rs", "fixtures/epic/tokens/grok"],
    "input": "trimmed real summary.json of interactive probe session 01a1bd42-396e-7cb3-9e06-4c45a5eda666 and subagent session 01a1bd47-d8a4-7a30-ae61-170f1a6f55cd under a temp GROK_HOME; boundaries: the subagent session is ignored, an older mtime, a foreign cwd, an info.id differing from its directory, a non-UUID directory, two fresh top-level sessions (ambiguous) and an absent home all give no session; require_captured_session accepts only the exact id; group() of '/private/tmp/grok enc_~+@,=#%x.y-z' equals the observed '%2Fprivate%2Ftmp%2Fgrok%20enc_~%2B%40%2C%3D%23%25x.y-z'; GROK_HOME blank falls back to ~/.grok"
  },
  {
    "id": "s4",
    "title": "Grok token row from the session's usage.json",
    "check": "cargo test -p toolu-epic-orchestrator --lib metrics::",
    "depends_on": ["s3"],
    "ac_refs": ["AC-4"],
    "paths": ["crates/epic-orchestrator/src/metrics/tokens.rs", "crates/epic-orchestrator/src/metrics/tests", "crates/epic-orchestrator/src/launch/grok_session.rs", "fixtures/epic/tokens/grok", "fixtures/epic/tokens/state"],
    "input": "real usage.json of a probe session whose subagent calls are rolled into the parent total (modelCalls 5 = 2 own + 3 child), and an issue record naming it; boundaries: no usage.json is unavailable with its reason, unparseable JSON is unavailable"
  },
  {
    "id": "s5",
    "title": "Grok shutdown sends ctrl+c then /exit",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::lifecycle",
    "depends_on": ["s1"],
    "ac_refs": ["AC-6"],
    "paths": ["crates/epic-orchestrator/src/launch/lifecycle.rs", "crates/epic-orchestrator/src/launch/tests/lifecycle_test.rs"],
    "input": "scripted herdr socket rig with a started grok agent; boundary: the claude sequence still starts with esc"
  },
  {
    "id": "s6",
    "title": "Grok in limit hosts, status host rows and the committed live probe evidence",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- limit:: probe_evidence:: status_resources::",
    "depends_on": ["s1"],
    "ac_refs": ["AC-5"],
    "paths": ["crates/epic-orchestrator/src/limit.rs", "crates/epic-orchestrator/src/status_resources.rs", "crates/epic-orchestrator/src/probe_evidence.rs", "crates/epic-orchestrator/src/tests", "docs/toolu/evidence/epic-hosts-v1.json"],
    "input": "the live Grok probe record (start, tool task, resume, background work, cancel/replace, teardown), a temp resource policy with a grok cap, a rate-limited failed report from a grok worker; boundary: evidence lacking hosts.grok is invalid"
  },
  {
    "id": "s7",
    "title": "Local merge gate and status plan read a grok worker's ledger under Claude Code's directory",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- gate::local:: status_plan::",
    "depends_on": ["s1"],
    "ac_refs": ["AC-8"],
    "paths": ["crates/epic-orchestrator/src/gate/local.rs", "crates/epic-orchestrator/src/gate/tests/local_test.rs", "crates/epic-orchestrator/src/status_plan.rs", "crates/epic-orchestrator/src/tests/status_plan_test.rs", "crates/epic-orchestrator/src/hosts.rs"],
    "input": "a real temp Git worktree on a branch with a ledger at .claude/tmp/plan-ledger/<branch>.json, as toolu wrote it inside a live Grok pane; boundary: an unknown host still says 'no host recorded'"
  },
  {
    "id": "s8",
    "title": "Live rig launch --kind grok records running with the captured Grok session",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::deliver::tests::",
    "depends_on": ["s3"],
    "ac_refs": ["AC-9"],
    "paths": ["crates/epic-orchestrator/src/launch/deliver.rs", "crates/epic-orchestrator/src/launch/tests/deliver_test.rs", "crates/epic-orchestrator/src/tests/launch_rig_test.rs", "crates/epic-orchestrator/src/launch/grok_session.rs"],
    "input": "launch rig: origin-backed checkout, herdr socket fixture, an agent shell script writing $GROK_HOME/sessions/<encoded $PWD>/<uuid>/summary.json; observed: the issue record has stage running, kind grok and session_id equal to that uuid, and the agent.start call the socket fixture recorded has kind grok with args starting --always-approve; boundary: an agent that writes no session fails with cannot attest grok session identity"
  },
  {
    "id": "s9",
    "title": "CLI: route --hosts grok, launch --kind grok, probe evidence, help texts and status schema; regenerate CLI docs and contract snapshot",
    "check": "TOOLU_RESOURCE_HOME=\"$(mktemp -d)\" cargo test -p toolu-cli --test epic_graph --test epic_launch --test epic_probe --test contract && cargo xtask gate --only docs-cli",
    "depends_on": ["s1", "s2", "s3", "s5", "s6"],
    "ac_refs": ["AC-1", "AC-2", "AC-5"],
    "paths": ["crates/cli/tests/epic_graph.rs", "crates/cli/tests/epic_launch.rs", "crates/cli/tests/epic_probe.rs", "crates/cli/tests/contract.rs", "crates/cli/tests/fixtures/commands_json.snap", "crates/epic-orchestrator/src/verbs_actions/launch.rs", "crates/epic-orchestrator/src/verbs_hosts.rs", "crates/epic-orchestrator/src/status_schema.rs", "docs/cli"],
    "input": "real toolu binary with fixtures/epic/epic248-graph.json: route --hosts grok:1 --dry-run, launch --kind grok --dry-run fresh and resume with a captured grok session; boundary: --hosts nope:1 lists grok in its error"
  },
  {
    "id": "s10",
    "title": "User docs: epic-orchestrator README, docs README, SKILL host list, worker-brief skill syntax, preflight Grok row, fixtures README",
    "check": "cargo xtask check-markdown-cli && cargo test -p toolu-cli --test epic_markdown --test epic_skill",
    "depends_on": ["s9"],
    "ac_refs": ["AC-1", "AC-2", "AC-4", "AC-5"],
    "paths": ["plugins/epic-orchestrator", "docs/epic-orchestrator", "fixtures/README.md", "crates/cli/tests/epic_markdown.rs", "crates/cli/tests/epic_skill.rs"],
    "input": "the shipped argv, routing defaults, token source, the Grok login check and the SessionStart-hook limitation"
  },
  {
    "id": "s11",
    "title": "Gate: every step but tests and coverage, plus the changed packages' tests",
    "check": "cargo xtask gate --only gate-change --only fmt --only clippy --only guardrails --only layers --only reach --only deny --only machete --only unused-pub --only hooks --only workflows --only docs-cli --only cli-compat --only jscpd --only rust-quality --only docs --only context-budget --only final-removal --only gate-coverage --only packaging --only ci-paths --only pack && TOOLU_RESOURCE_HOME=\"$(mktemp -d)\" cargo test -p toolu-epic-orchestrator && TOOLU_RESOURCE_HOME=\"$(mktemp -d)\" cargo test -p toolu-cli --test contract --test epic_finish --test epic_graph --test epic_launch --test epic_markdown --test epic_metrics --test epic_probe --test epic_report --test epic_skill --test epic_token",
    "depends_on": ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8", "AC-9"],
    "paths": ["crates", "fixtures", "docs", "plugins", "Cargo.toml", "Cargo.lock"],
    "input": "the whole branch diff"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/hosts.rs`, `tests/hosts_test.rs`
- `crates/epic-orchestrator/src/route/table.rs`, `route/tests/table_test.rs`
- `crates/epic-orchestrator/src/launch.rs`, `launch/grok_session.rs` (new),
  `launch/tests/grok_session_test.rs` (new), `launch/session.rs`,
  `launch/screen.rs`, `launch/lifecycle.rs`, `launch/tests/lifecycle_test.rs`,
  `launch/tests/deliver_test.rs`
- `crates/epic-orchestrator/src/metrics/tokens.rs`, `metrics/tests/tokens_test.rs`
- `crates/epic-orchestrator/src/limit.rs`, `probe_evidence.rs`,
  `status_resources.rs` tests, `tmux_backend.rs`, `gate/local.rs`,
  `status_plan.rs`, `verbs_hosts.rs`, `verbs_actions/launch.rs`,
  `status_schema.rs`, `tests/launch_rig_test.rs`
- `fixtures/epic/hosts.json`, `fixtures/index.json`,
  `fixtures/epic/tokens/grok/…` (new), `fixtures/README.md`
- `docs/toolu/evidence/epic-hosts-v1.json`, `docs/cli/`,
  `crates/cli/tests/{epic_graph,epic_launch,epic_probe}.rs`,
  `crates/cli/tests/fixtures/commands_json.snap`
- `plugins/epic-orchestrator/README.md`, `skills/epic-orchestrator/SKILL.md`,
  `references/worker-brief.md`, `references/preflight.md`,
  `docs/epic-orchestrator/README.md`

## Verification

- End to end: `toolu epic route --hosts grok:1 --dry-run` and
  `launch --kind grok --dry-run` with the built binary on the #248 graph. The
  rig launch (s8) proves a live `running` record with the captured session.
  The committed probe evidence (s6) holds the real herdr and Grok lifecycle.
- Failure and boundary cases are listed per step: an ambiguous, foreign,
  subagent or old session gives no capture; a missing usage record is
  `unavailable`; an unknown host stays refused.
- Documentation: s9 (CLI reference, status schema) and s10 (README, SKILL,
  references, fixtures README) with their checks.
- Full gate: after the scoped commit, `DOCKER_CONTEXT=colima-toolu-gate cargo
  xtask deploy-gate` runs the complete `cargo xtask gate` in the clean
  container. The host cannot pass the `tests`/`coverage` steps (Homebrew
  toolu and the live engine; see the local gate notes), so s11 is the ledger's
  host-runnable gate.
- Final: `toolu ledger run docs/toolu/plans/2026-11-08-epic-grok-worker-host.md
  --verify`, `code-review:review`, `toolu ledger verdict status` reporting
  `overall: ready`, push, a PR against `main` that refers to #846 without
  closing it, then `pr-babysit:babysit`.

## Plan review

```
s4: 🟡 should-fix: adding a grok issue record to fixtures/epic/tokens/state can change summary and token counts in other metrics tests. Declared crates/epic-orchestrator/src/metrics/tests. Fixed.
s8: 🟡 should-fix: AC-9's observed results (record fields, recorded agent.start argv) were implicit. Named them in the step input. Fixed.
Verification: 🔵 consider: the host cannot pass the gate's tests/coverage steps (Homebrew toolu, live engine). s11 is the host-runnable gate, and the full gate runs in the deploy-gate container after the scoped commit. Recorded.
```

Jev (plan review): every AC proven by a step 0.46, then weakest AC-9 (0.36),
sharpened in s8.

## Deviations

- s6: the probe-evidence error lists the expected host keys sorted
  (`claude, codex, cursor, grok, muse`); the spec's AC-5 boundary text was
  corrected to the observed order.
- s9: `epic_graph`'s Muse route test became one loop over `muse` and `grok`,
  and `epic_launch`'s Muse resume test one table over both hosts, so the new
  cases add no copied blocks.
- s5: the Muse shutdown test's key collection moved into a `typed` helper
  that the Grok test shares.
