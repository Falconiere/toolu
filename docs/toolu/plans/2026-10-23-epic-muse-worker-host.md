# Epic: Muse as a worker host — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-epic-muse-worker-host-design.md   **Topic:** launch, route, capture, meter and tear down Muse Code workers

## Evidence and approach

Inspected `hosts.rs`, `route/table.rs`, `route/pool.rs`, `launch/session.rs`,
`launch/lifecycle.rs`, `launch/deliver.rs`, `metrics/tokens.rs`, `limit.rs`,
`status_resources.rs`, `probe_evidence.rs` and their tests, the shared
`fixtures/epic/hosts.json` and `fixtures/epic/tokens/`, the scripted herdr rig
(`herdr_state`, `launch_rig`), and the CLI tests `epic_graph`, `epic_launch`,
`epic_probe`. A live Muse 1.4.12 / herdr 0.9.5 probe supplied the argv, the
session layout, the usage records, the exit dialog and resume behaviour.

Adding `HostKind::Muse` makes the compiler list every `match`. Session capture
and token reading for Muse get their own modules (`launch/muse_session.rs`,
`metrics/muse_tokens.rs`) so `session.rs` and `tokens.rs` stay under the 300
code-line limit. Fixtures are trimmed copies of the probe's real session log
(`01a16abb-7eef-7340-8cf2-2e0378dcf66c`) and one of its subagent logs.

## Workstream summary

hosts → routing → session capture → tokens → exit → limit/status/evidence →
CLI and generated docs → user docs → gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "HostKind::Muse: argv, skill_ref, error text, HOST_LIMIT Muse lines; fixture cases",
    "check": "cargo test -p toolu-epic-orchestrator --lib hosts::tests",
    "ac_refs": ["AC-2", "AC-7"],
    "paths": ["crates/epic-orchestrator/src/hosts.rs", "crates/epic-orchestrator/src/tests/hosts_test.rs", "fixtures/epic/hosts.json"],
    "input": "live argv from herdr agent start --kind muse (fresh, resume <uuid>, resume --last, attended); Muse 1.4.12 throttle strings; boundaries: the quota warning does not match, a model with a space is refused, opencode/gemini stay unknown with muse listed"
  },
  {
    "id": "s2",
    "title": "Muse routing defaults per tier and routing.json override",
    "check": "cargo test -p toolu-epic-orchestrator --lib route::table",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1"],
    "paths": ["crates/epic-orchestrator/src/route/table.rs", "crates/epic-orchestrator/src/route/tests/table_test.rs"],
    "input": "default table and a temp routing.json with hosts.muse; boundary: a routing.json without muse keeps the Muse defaults"
  },
  {
    "id": "s3",
    "title": "Muse session capture from the real day/uuid layout",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::",
    "depends_on": ["s1"],
    "ac_refs": ["AC-3"],
    "paths": ["crates/epic-orchestrator/src/launch/muse_session.rs", "crates/epic-orchestrator/src/launch/tests/muse_session_test.rs", "crates/epic-orchestrator/src/launch/session.rs", "crates/epic-orchestrator/src/launch.rs", "fixtures/epic/muse"],
    "input": "trimmed real session.jsonl of probe session 01a16abb-7eef-7340-8cf2-2e0378dcf66c laid out under a temp XDG_DATA_HOME; boundaries: older mtime, foreign workspace_root, stream.id differing from its directory, non-UUID directory, two fresh sessions (ambiguous), absent data root; require_captured_session accepts only the exact id"
  },
  {
    "id": "s4",
    "title": "Muse token row from main and subagent session logs",
    "check": "cargo test -p toolu-epic-orchestrator --lib metrics::",
    "depends_on": ["s3"],
    "ac_refs": ["AC-4"],
    "paths": ["crates/epic-orchestrator/src/metrics/muse_tokens.rs", "crates/epic-orchestrator/src/metrics/tests/muse_tokens_test.rs", "crates/epic-orchestrator/src/metrics/tokens.rs", "crates/epic-orchestrator/src/metrics/tests/tokens_test.rs", "crates/epic-orchestrator/src/metrics.rs", "fixtures/epic/tokens", "fixtures/epic/muse"],
    "input": "real goal_usage_attribution records of the probe session and one subagent log, plus one repeated usage_id; issue record e.json; boundaries: unreported zero records ignored, missing session directory is unavailable"
  },
  {
    "id": "s5",
    "title": "Muse shutdown sends enter after /exit and tolerates a gone agent",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::lifecycle",
    "depends_on": ["s1"],
    "ac_refs": ["AC-6"],
    "paths": ["crates/epic-orchestrator/src/launch/lifecycle.rs", "crates/epic-orchestrator/src/launch/tests/lifecycle_test.rs"],
    "input": "scripted herdr socket rig with a started muse agent; boundary: the agent is already gone when enter is sent and the outcome is still exited; claude sequence unchanged"
  },
  {
    "id": "s6",
    "title": "Muse in limit hosts, status host rows and the committed live probe evidence",
    "check": "cargo test -p toolu-epic-orchestrator --lib -- limit probe_evidence status_resources status",
    "depends_on": ["s1"],
    "ac_refs": ["AC-5"],
    "paths": ["crates/epic-orchestrator/src/limit.rs", "crates/epic-orchestrator/src/status_resources.rs", "crates/epic-orchestrator/src/probe_evidence.rs", "crates/epic-orchestrator/src/tests", "docs/toolu/evidence/epic-hosts-v1.json"],
    "input": "live Muse probe record (start, tool task, resume, background work, cancel/replace, teardown), a temp resource policy with a muse cap, a rate-limited failed report from a muse worker; boundary: evidence lacking hosts.muse is invalid"
  },
  {
    "id": "s7",
    "title": "CLI: route --hosts muse, launch --kind muse, probe evidence; regenerate CLI docs and contract snapshot",
    "check": "cargo test -p toolu-cli --test epic_graph --test epic_launch --test epic_probe --test contract",
    "depends_on": ["s1", "s2", "s3", "s5", "s6"],
    "ac_refs": ["AC-1", "AC-2", "AC-5"],
    "paths": ["crates/cli/tests/epic_graph.rs", "crates/cli/tests/epic_launch.rs", "crates/cli/tests/epic_probe.rs", "crates/cli/tests/fixtures/commands_json.snap", "crates/epic-orchestrator/src/verbs_actions/launch.rs", "docs/cli"],
    "input": "real toolu binary with the epic graph fixture: route --hosts muse:1 --dry-run, launch --kind muse --dry-run fresh and resume with a captured muse session; boundary: --hosts nope:1 lists muse in its error"
  },
  {
    "id": "s8",
    "title": "User docs: epic-orchestrator README, docs README, SKILL host list, worker-brief skill syntax",
    "check": "cargo xtask check-markdown-cli",
    "depends_on": ["s7"],
    "ac_refs": ["AC-1", "AC-2", "AC-4", "AC-5"],
    "paths": ["plugins/epic-orchestrator", "docs/epic-orchestrator"],
    "input": "the shipped argv, routing defaults and token source"
  },
  {
    "id": "s9",
    "title": "Full gate",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"],
    "input": "whole workspace"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/hosts.rs`, `tests/hosts_test.rs`, `fixtures/epic/hosts.json`
- `crates/epic-orchestrator/src/route/table.rs`, `route/tests/table_test.rs`
- `crates/epic-orchestrator/src/launch/muse_session.rs` (new), `launch/tests/muse_session_test.rs` (new), `launch/session.rs`, `launch.rs`
- `crates/epic-orchestrator/src/metrics/muse_tokens.rs` (new), `metrics/tests/muse_tokens_test.rs` (new), `metrics/tokens.rs`, `metrics.rs`
- `crates/epic-orchestrator/src/launch/lifecycle.rs`, `launch/tests/lifecycle_test.rs`
- `crates/epic-orchestrator/src/{limit,status_resources,probe_evidence}.rs` and tests
- `docs/toolu/evidence/epic-hosts-v1.json`
- `fixtures/epic/muse/` (new), `fixtures/epic/tokens/state/issues/e.json` (new)
- `crates/cli/tests/{epic_graph,epic_launch,epic_probe}.rs`, `docs/cli/`, `crates/cli/tests/fixtures/commands_json.snap`
- `plugins/epic-orchestrator/README.md`, `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`, `references/worker-brief.md`, `docs/epic-orchestrator/README.md`

## Verification

`toolu epic route --hosts muse:1 --dry-run` and `toolu epic launch --kind muse
--dry-run` against the real graph print Muse routes and argv; session capture and
token tests read trimmed real Muse logs; the probe evidence validates with Muse;
`cargo xtask gate` passes. Docs and generated CLI reference are regenerated and
checked by `check-markdown-cli` and the contract test.

## Plan review

```
Steps: 🟡 should-fix: step inputs named only happy-path data. Added each AC's boundary case to its step input. Fixed.
s7: 🔵 consider: depends on s1 and s5 as well (argv and exit behaviour surface in the CLI). Fixed.
```

Jev (plan review): AC/step alignment 0.44 before the boundary inputs were added, 0.54 after (Jev sees only step text; each AC has a step whose check exercises its boundary);
dependency order 0.82.
