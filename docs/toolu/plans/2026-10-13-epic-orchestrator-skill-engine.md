# Orchestrator skill, worker brief and commands for the engine (#452) — Plan

**Date:** 2026-10-13   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-13-epic-orchestrator-skill-engine-design.md   **Topic:** rewrite the epic-orchestrator Markdown for the resident engine

## Evidence and approach

Markdown only (spec Architecture; Jev 0.99). Reused contracts:
`crates/cli/tests/epic_markdown.rs` (verbs of the real binary, rendered brief
per host and tracker), `crates/epic-orchestrator/src/launch/tests/brief_test.rs`
(placeholders, per-host skill references), `crates/xtask/src/context_budget.rs`
(`SKILLS` rows), `tools/toolu-opencode/scripts/lib/opencode-port.ts` and
`scripts/__tests__/epic-surfaces.test.ts` (the OpenCode port and its audit),
`cargo xtask check-markdown-cli` (drift; it judges inline spans that start
with `toolu` and every command in shell fences, so `herdr` stays in inline
spans: it is not on the `external` list). Engine facts the
Markdown states: `watch.rs` `waiting` (PR watched from `pr-open`), `babysit.rs`
(green tick sets `ready`), `pr-babysit` `link.rs` (tick prompt text),
`query.rs` `wait_default` (2700 s Claude, 480 s elsewhere).

## Workstream summary

skill + its contract tests → worker brief + its tests → recovery and command
→ context budget → OpenCode port and regenerated tree → evals and project
skill → READMEs → live wait evidence per host → eval runs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Rewrite SKILL.md for the engine (<150 lines) and extend epic_markdown contracts: references in surfaces, line limit, no gate/launch/finish/merge in the skill, judgment table complete, preflight via toolu doctor",
    "check": "cargo test -p toolu-cli --test epic_markdown",
    "ac_refs": ["AC-1", "AC-2", "AC-4"],
    "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "crates/cli/tests/epic_markdown.rs", "crates/cli/tests/helpers/**"],
    "input": "the shipped SKILL.md and the real toolu binary's epic --help",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Worker brief: report babysit then stop; tick-result, REBASE and STATUS? prompts; engine sets ready; brief tests per host and tracker",
    "check": "cargo test -p toolu-epic-orchestrator brief && cargo test -p toolu-cli --test epic_markdown",
    "ac_refs": ["AC-3"],
    "depends_on": ["S1"],
    "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md", "crates/epic-orchestrator/src/launch/brief.rs", "crates/epic-orchestrator/src/launch/tests/brief_test.rs", "crates/cli/tests/epic_markdown.rs"],
    "input": "graph_248 fixture and the real binary's launch --dry-run brief for claude, codex, cursor, opencode on github, jira, linear",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Rewrite references/recovery.md around status, journal and doctor; commands/epic.md subcommands start/status/pause/resume/stop/serve",
    "check": "cargo test -p toolu-cli --test epic_markdown && cargo xtask check-markdown-cli",
    "ac_refs": ["AC-2", "AC-7", "AC-8"],
    "depends_on": ["S1"],
    "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md", "plugins/epic-orchestrator/commands/epic.md", "crates/cli/tests/epic_markdown.rs", "docs/cli/commands.json", "tooling/conventions/markdown-cli.json", "tooling/conventions/markdown-cli/**"],
    "input": "the shipped recovery.md and epic.md",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Add the epic-orchestrator description row (80 words, trigger phrases) to the context-budget gate",
    "check": "cargo test -p xtask context_budget && cargo xtask gate --only context-budget",
    "ac_refs": ["AC-1"],
    "depends_on": ["S1"],
    "paths": ["crates/xtask/src/context_budget.rs", "crates/xtask/src/tests/context_budget_test.rs", "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "plugins/*/skills/**/SKILL.md", "plugins/toolu/hooks/docs/**"],
    "input": "the shipped skill descriptions",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "OpenCode port of the wait paragraph, regenerate the generated tree, update epic-surfaces.test.ts",
    "check": "bun run check:opencode-surface && bun test tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts",
    "ac_refs": ["AC-6"],
    "depends_on": ["S1", "S2", "S3"],
    "paths": ["plugins/epic-orchestrator/**", "tools/toolu-opencode/scripts/**", "tools/toolu-opencode/generated/**", "plugins/*/skills/**", "plugins/*/commands/**", "plugins/*/agents/**", "plugins/*/.claude-plugin/plugin.json"],
    "input": "the regenerated OpenCode surface",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Evals for work/dry-run/where/parallel/stop; verify-epic-watcher runs engine commands incl. toolu epic wait",
    "check": "jq -e '[.evals[].name] | length == 5' plugins/epic-orchestrator/skills/epic-orchestrator/evals/evals.json && grep -q 'toolu epic wait' .toolu/skills/verify-epic-watcher/SKILL.md && ! grep -Eq 'epic-watch|\\.ts\\b' .toolu/skills/verify-epic-watcher/SKILL.md",
    "ac_refs": ["AC-9", "AC-10"],
    "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/evals/evals.json", ".toolu/skills/verify-epic-watcher/SKILL.md"],
    "input": "the shipped evals.json and project skill",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "READMEs: waiting per host, idle workers after babysit, /epic subcommands",
    "check": "cargo xtask check-markdown-cli && ! grep -q 'On every host the orchestrator waits in the foreground' plugins/epic-orchestrator/README.md",
    "ac_refs": ["AC-2"],
    "depends_on": ["S1", "S2"],
    "paths": ["plugins/epic-orchestrator/README.md", "docs/epic-orchestrator/README.md", "docs/cli/commands.json", "tooling/conventions/**"],
    "input": "the two READMEs",
    "model": "inherit"
  },
  {
    "id": "S8",
    "title": "Live wait evidence: Claude Code background Bash, a codex exec session and an opencode run session each see an event after several bounded iterations against an isolated engine",
    "check": "test -s docs/toolu/evidence/2026-10-13-epic-wait-hosts.md && grep -q 'claude: PASS' docs/toolu/evidence/2026-10-13-epic-wait-hosts.md && grep -q 'codex: PASS' docs/toolu/evidence/2026-10-13-epic-wait-hosts.md && grep -q 'opencode: PASS' docs/toolu/evidence/2026-10-13-epic-wait-hosts.md",
    "ac_refs": ["AC-5"],
    "depends_on": ["S1"],
    "paths": ["docs/toolu/evidence/2026-10-13-epic-wait-hosts.md"],
    "input": "isolated TOOLU_RESOURCE_HOME, real engine from target/debug/toolu, a needs-human report",
    "model": "inherit"
  },
  {
    "id": "S9",
    "title": "Run the read-only evals and stop (isolated engine) with one subagent each; record grades",
    "check": "test -s docs/toolu/evidence/2026-10-13-epic-orchestrator-evals.md && ! grep -q 'FAIL' docs/toolu/evidence/2026-10-13-epic-orchestrator-evals.md",
    "ac_refs": ["AC-10"],
    "depends_on": ["S1", "S3", "S6"],
    "paths": ["docs/toolu/evidence/2026-10-13-epic-orchestrator-evals.md"],
    "input": "Falconiere/toolu#402 (read-only), isolated engine",
    "model": "sonnet"
  },
  {
    "id": "S10",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-6"],
    "depends_on": ["S1", "S2", "S3", "S4", "S5", "S6", "S7"],
    "paths": ["**"],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/{worker-brief,recovery}.md`
- `plugins/epic-orchestrator/commands/epic.md`
- `plugins/epic-orchestrator/skills/epic-orchestrator/evals/evals.json`
- `.toolu/skills/verify-epic-watcher/SKILL.md`
- `crates/cli/tests/epic_markdown.rs`, `crates/epic-orchestrator/src/launch/tests/brief_test.rs`
- `crates/xtask/src/context_budget.rs`, `crates/xtask/src/tests/context_budget_test.rs`
- `tools/toolu-opencode/scripts/lib/opencode-port.ts`, `tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts`, `tools/toolu-opencode/generated/**`
- `plugins/epic-orchestrator/README.md`, `docs/epic-orchestrator/README.md`
- `docs/toolu/evidence/2026-10-13-epic-wait-hosts.md`, `docs/toolu/evidence/2026-10-13-epic-orchestrator-evals.md`

## Verification

End to end: the real binary renders every brief, the drift gate reads every
command, the OpenCode tree regenerates without drift, and a real engine
answers a background `toolu epic wait` on Claude Code and a bounded loop on
Codex and OpenCode. Failure checks: the contract tests fail on a banned verb in
the skill, a placeholder left in a brief, or a missing judgment kind; the
budget row fails on a description over 80 words. Docs: both READMEs.
Delivery: scoped commits per step (pushed once the step's check is green), then
`bun plugins/toolu/hooks/dist/plan-ledger.js run <plan> --verify`, the
`toolu-review:review` state, `verdict.js status` = ready, push, PR, then
`report babysit` and the brief's babysit handoff.

## Plan review (round 1)

- Evidence: 🟡 should-fix: claimed a per-plugin `herdr` allowlist that does not exist. Fixed: `herdr` stays in inline spans.
- Verification: 🟡 should-fix: no delivery tail. Fixed: ledger `--verify`, review state, verdict, PR, babysit.
- AC coverage: every spec AC (AC-1…AC-10) maps to a step; no dangling ref.
- S8/S9: live checks read evidence files whose content they assert (PASS lines, no FAIL); their inputs are isolated so they cannot touch the live epic.

**Status:** Approved
