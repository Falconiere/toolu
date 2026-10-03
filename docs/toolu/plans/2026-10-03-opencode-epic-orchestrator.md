# OpenCode: epic-orchestrator native agent routing and worktrees — Plan

**Date:** 2026-10-03   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-03-opencode-epic-orchestrator-design.md   **Topic:** OpenCode workers get resolvable skill ids, a 1.x version guard, provider/model routing and a clean worktree; the generated orchestrator skill loses its Claude-only steps; proven hermetically and live on the pinned host (#356, OP-22)

## Evidence and approach

- **Inspected:**
  - `plugins/epic-orchestrator/scripts/{hosts,launch-issue,common,route,checkpoint,finish-issue,epic-watch,report}.ts` and `trackers/jira.ts`;
  - their tests (`hosts`, `launch-issue`, `skill-contract`, `epic-watch`, `route`, `jira-tracker`);
  - the source `SKILL.md`, `references/worker-brief.md` and `recovery.md`;
  - `tools/toolu-opencode/scripts/lib/opencode-port.ts`, `scripts/__tests__/{surface-audit,delivery-surfaces}.ts`;
  - `src/adapter/tool-before.ts`: `task` forwards `model` to agent-tier, and `tool-routes.test.ts` proves the block;
  - `src/plugin/__tests__/{workflow,jev,core,delivery}-fixtures.ts` and the delivery live test;
  - `contract/capability-matrix.json`.
- **Live probes (pinned 1.18.34):** the TUI flags `--auto`, `-m`, `--continue` and `/exit`; `--continue` resumes per worktree directory; bash tool timeout 120000 default, 600000 maximum. Separately, a non-forced `git worktree remove` succeeds with only excluded files.
- **Approach (spec):** see the spec's Architecture, items 1–7. Decisions checked with Jev: keep herdr (0.99), OpenCode-only exclude (0.97), foreground watcher (0.98), version guard (0.76).
- **Recall:** comemory's OpenCode leaf-startup and Bun `.env` notes. They explain why the Jira line runs `"$TOOLU_BUN" --no-env-file`.

## Workstream summary

hosts → launcher and worktree exclude → helper lookup → watcher signals → source skill → OpenCode port and surface audit → capability matrix → hermetic workflow → docs → live worker → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "hosts",
    "title": "hosts.ts: OPENCODE_SKILL_IDS + skill({ name }) form (unmapped pair throws), provider/model check in agentArgs(\"opencode\"), opencodeVersionProblem; fix the #variant comment; hosts.test.ts cross-checks ids against generated/skills frontmatter and keeps Claude/Codex/Cursor outputs",
    "ac_refs": ["AC-1", "AC-3", "AC-10"],
    "paths": [
      "plugins/epic-orchestrator/scripts/**",
      "tools/toolu-opencode/generated/skills/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "The committed generated/skills SKILL.md frontmatter; version strings 1.18.34, 2.0.21 and empty",
    "check": "bun test plugins/epic-orchestrator/scripts/__tests__/hosts.test.ts",
    "model": "inherit"
  },
  {
    "id": "launch",
    "title": "launch-issue.ts: export START_PROMPT, OPENCODE_EXCLUDE, excludeOpencodeState; OpenCode model/version refusal before any side effect; exclude after the worktree and before agent start (dry-run logs only); OpenCode Jira read line; launch-issue.test.ts dry-run/refusal cases and worktree-exclude.test.ts",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-10"],
    "depends_on": ["hosts"],
    "paths": [
      "plugins/epic-orchestrator/scripts/**",
      "tools/toolu-opencode/generated/skills/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "The #248 fixture through launch-issue.ts --dry-run in a sandbox state dir (kinds opencode, claude, codex, cursor; models probe/scripted and sonnet), asserting the exact start line, the exclude log line only for opencode, and a brief with skill({ name }) ids and no /x: or $x: invocations; renderBrief for a Jira-tracker graph with kind opencode (the $TOOLU_BUN jira.sh line); a non-dry opencode launch with PATH holding only Bun's directory (exit 1, no issues/<key>.json); a sandbox git repo with a linked worktree holding .opencode/toolu/state/x, .opencode/tmp/y, an untracked .opencode/toolu/plugins.json and a source edit, with excludeOpencodeState run twice, once with info/ absent and once on an exclude file lacking a trailing newline, then snapshotted by the real checkpoint.ts and removed by a non-forced git worktree remove",
    "check": "bun test plugins/epic-orchestrator/scripts/__tests__/launch-issue.test.ts plugins/epic-orchestrator/scripts/__tests__/worktree-exclude.test.ts plugins/epic-orchestrator/scripts/__tests__/checkpoint.test.ts plugins/epic-orchestrator/scripts/__tests__/finish-issue.test.ts",
    "model": "inherit"
  },
  {
    "id": "helpers",
    "title": "common.ts helperCandidates (TOOLU_CONFIG_DIR, then CLAUDE_CONFIG_DIR|~/.claude, then CODEX_HOME|~/.codex) used by route.ts jevScript and trackers/jira.ts jiraScript after their EPIC_* override; tests",
    "ac_refs": ["AC-5"],
    "paths": [
      "plugins/epic-orchestrator/scripts/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "Sandbox config dirs holding real executable jev/jev.sh and jira/jira.sh files, each present in only one root",
    "check": "bun test plugins/epic-orchestrator/scripts/__tests__/route.test.ts plugins/epic-orchestrator/scripts/__tests__/jira-tracker.test.ts",
    "model": "inherit"
  },
  {
    "id": "watch",
    "title": "epic-watch.test.ts: an opencode record's rate-limited failed report raises failed and host-limited (host opencode, cooldown true, hosts.json entry); a running opencode record whose agent left the herdr list raises gone once",
    "ac_refs": ["AC-8"],
    "paths": [
      "plugins/epic-orchestrator/scripts/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "A sandbox state dir with an opencode issue record and status file, agent maps with and without the worker",
    "check": "bun test plugins/epic-orchestrator/scripts/__tests__/epic-watch.test.ts",
    "model": "inherit"
  },
  {
    "id": "skill-source",
    "title": "Source SKILL.md: OpenCode 1.x preflight, worktree selection note, launch step 4 version check and exclude; worker-brief.md maintainer comment names the skill({ name }) form",
    "ac_refs": ["AC-10"],
    "depends_on": ["launch"],
    "paths": [
      "plugins/epic-orchestrator/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "The edited source skill and brief against the launcher's placeholder list",
    "check": "bun test plugins/epic-orchestrator/scripts/__tests__/skill-contract.test.ts && rg -q 'opencode --version' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md",
    "model": "inherit"
  },
  {
    "id": "port",
    "title": "OPENCODE_PORTS entries for epic-orchestrator SKILL.md (ROOT from TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?, skill-list preflight with generated ids and no Codex install, foreground watcher --max-wait 480 with bash timeout 600000, header and Stop line); regenerate generated/; epic-surfaces.test.ts: the generated SKILL.md, its references and the epic command carry none of run_in_background, end your turn, re-invoked, CLAUDE_PLUGIN_ROOT, ${PLUGIN_ROOT, npx @toolu/plugins, delivery-flow:delivery-flow, pr-babysit:babysit, toolu: or a double-hyphen skill id; every skill({ name }) and \"$S/<script>\" resolves; the watcher, timeout and root lines are present",
    "ac_refs": ["AC-6"],
    "depends_on": ["skill-source"],
    "paths": [
      "plugins/**",
      "docs/**",
      "README.md",
      "LICENSE",
      "tooling/conventions/**",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/inventory/**",
      "tools/toolu-opencode/src/surfaces/**",
      "tools/toolu-opencode/src/bootstrap/**",
      "tooling/src/**",
      ".claude-plugin/marketplace.json",
      "tools/toolu-conformance/src/**"
    ],
    "input": "The real plugins/epic-orchestrator sources through planSurface; the committed generated tree; a stagePlugins copy; a sandbox copy of the plugin with an epic port anchor removed and duplicated",
    "check": "bun test --timeout 120000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts tooling/src/__tests__/bundle-plugins.test.ts && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "matrix",
    "title": "capability-matrix.json: epic-orchestrator task axis describes herdr worktree sessions whose delegation goes through task; regenerate docs/opencode-host-contract.md",
    "ac_refs": ["AC-10"],
    "paths": [
      "tools/toolu-opencode/contract/**",
      "docs/opencode-host-contract.md",
      "docs/portable-core.md",
      "tooling/src/**",
      "plugins/**",
      "tools/toolu-opencode/package.json"
    ],
    "input": "The committed probe evidence and the corrected matrix",
    "check": "bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "workflows",
    "title": "epic-workflows.test.ts via createTooluHooks in a sandbox git project selecting only epic-orchestrator: config registers the orchestrator and four delivery-chain skills; the generated ROOT/S lines run in the shell.env bash and report.ts writes execution; a native task records delegation telemetry; under agentTier block a task with a mismatched model is refused naming the step and the model-less call passes with step_id",
    "ac_refs": ["AC-7"],
    "depends_on": ["port"],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "tools/toolu-opencode/generated/**",
      "packages/toolu-core/src/**",
      "plugins/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "A sandbox git project with .opencode/toolu/plugins.json selecting epic-orchestrator, .opencode/toolu.config.json with agentTier block, and a real plan ledger whose running step declares model haiku",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/epic-workflows.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "plugins/epic-orchestrator/README.md OpenCode workers + foreground watcher + hosts table without #variant; docs/opencode.md Epic orchestrator subsection; docs/epic-orchestrator/README.md OpenCode line; regenerate",
    "ac_refs": ["AC-10"],
    "depends_on": ["port", "matrix"],
    "paths": [
      "plugins/**",
      "docs/**",
      "README.md",
      "LICENSE",
      "tooling/conventions/**",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/inventory/**"
    ],
    "input": "The edited docs and the regenerated tree",
    "check": "rg -q 'epic-orchestrator-epic-orchestrator' docs/opencode.md && rg -q 'info/exclude' plugins/epic-orchestrator/README.md && ! rg -q '#variant' plugins/epic-orchestrator/README.md && rg -q 'OpenCode' docs/epic-orchestrator/README.md && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "epic-worker.live.test.ts on the pinned host: real --version passes opencodeVersionProblem; worker in a linked worktree with agentArgs(\"opencode\") and START_PROMPT reads the brief, reports execution, loads delivery-flow-delivery-flow, delegates via task to toolu-quick-task, edits, is killed mid-turn (non-zero exit, status execution); checkpoint.ts snapshot has the edit and no state; --continue resumes the same session, commits, writes review state, pushes, reports ready; epic-watch --peek emits ready; porcelain has no toolu runtime path",
    "ac_refs": ["AC-3", "AC-9"],
    "depends_on": ["launch", "workflows", "docs"],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/contract/**",
      "packages/toolu-core/src/**",
      "plugins/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "Pinned opencode-ai@1.18.34 with the scripted loopback provider, an isolated profile, a git repo with a bare remote, a committed epic-orchestrator selection, and a brief rendered by renderBrief(..., \"opencode\")",
    "check": "TOOLU_LIVE_OPENCODE=1 bun test --timeout 900000 tools/toolu-opencode/src/plugin/__tests__/epic-worker.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate",
    "ac_refs": ["AC-10"],
    "depends_on": ["hosts", "launch", "helpers", "watch", "skill-source", "port", "matrix", "workflows", "docs", "live"],
    "paths": ["**"],
    "input": "The whole tree",
    "check": "env -u npm_config_store_dir TMPDIR=/private/tmp bun run test",
    "model": "inherit"
  }
]
```

## Deviations

- launch: `OPENCODE_EXCLUDE`, `excludeOpencodeState` and the version check live in a new `scripts/opencode-worker.ts`, not `launch-issue.ts`. The launcher had passed the 500-line lint limit. `hosts.ts` exports `opencodeModelArgs`, so the early model check and `agentArgs` share one rule.
- port: `stagePlugins` (the `@toolu/opencode` npm staging) shipped no epic-orchestrator scripts or reference templates. On an npm install, every `"$S/<script>"` and `launch-issue.ts`'s brief template were missing, so AC-6's "exists in a `stagePlugins` copy" could not hold. It now stages `scripts/*.ts`, `scripts/trackers/*.ts` and `skills/epic-orchestrator/references/*.md`, never tests or fixtures. `bundle-plugins.test.ts` dry-runs the staged `launch-issue.ts`, and `pack-inventory.ts` requires four of those files.
- launch: the refusal text omits "(tested on 1.18.34)". The plugin cannot read the contract pin, and a hard-coded patch version would drift.

## Plan review

- Spec AC-9: 🔴 blocker (fixed in the spec): `checkAcRefs` reported AC-9 as dangling because its `**AC-9 (live, …):**` heading is not a declared AC form. It now reads `**AC-9:**`, and `checkAcRefs` reports no dangling refs.
- launch: 🟡 should-fix (fixed): the input did not name the Jira brief, the forbidden invocation forms, the missing `info/` case, the case without a trailing newline, or the non-forced removal that AC-1 and AC-4 require. All are now stated.
- port: 🟡 should-fix (fixed): the title did not list AC-6's banned tokens or resolution checks. They are now stated.
- matrix: 🟡 should-fix (fixed): `check:opencode-host` also reads `docs/portable-core.md` and every plugin directory. Paths widened.
- live: 🟡 should-fix (fixed): the test uses `@toolu/conformance` harnesses; `tools/toolu-conformance/src/**` added.
- Jev alignment before the fixes: 1.52 of 2 (confidence 0.29). The gap choice was diffuse (AC coverage 0.35, paths 0.22, none 0.21), and inspection located the items above.
- Status: Approved.

## Critical files

- `plugins/epic-orchestrator/scripts/hosts.ts`, `launch-issue.ts`, `common.ts`, `route.ts`, `trackers/jira.ts`
- `plugins/epic-orchestrator/scripts/__tests__/hosts.test.ts`, `launch-issue.test.ts`, `route.test.ts`, `jira-tracker.test.ts`, `epic-watch.test.ts`, `worktree-exclude.test.ts` (new)
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`, `references/worker-brief.md`
- `tools/toolu-opencode/scripts/lib/opencode-port.ts`, `scripts/__tests__/epic-surfaces.test.ts` (new)
- `tools/toolu-opencode/src/plugin/__tests__/epic-workflows.test.ts`, `epic-worker.live.test.ts` (new)
- `tools/toolu-opencode/contract/capability-matrix.json`, `docs/opencode-host-contract.md`
- `tools/toolu-opencode/generated/**` (regenerated)
- `plugins/epic-orchestrator/README.md`, `docs/opencode.md`, `docs/epic-orchestrator/README.md`

## Verification

- **End to end:** a real OpenCode worker in an isolated epic worktree does the following, live on the pinned host:
  - loads the delivery chain from its brief;
  - delegates through native `task`;
  - survives cancellation through a snapshot and a per-worktree resume;
  - pushes, and reports `ready`, which the watcher sees.
- **Failure and boundary cases:**
  - a wrong model or a non-1.x/missing `opencode` refuses the launch with no side effect;
  - an unmapped skill pair throws;
  - a rate-limited `failed` report cools the `opencode` host;
  - a vanished worker raises `gone` once;
  - a mismatched native `task` model is refused under `agentTier: block`;
  - a missing or duplicated port anchor fails generation;
  - the exclude is idempotent and tolerates a missing `info/` and a missing trailing newline.
- **Existing hosts:** every existing epic-orchestrator test passes with only OpenCode expectations changed.
- **Docs:** the plugin README, `docs/opencode.md`, `docs/epic-orchestrator/README.md`, the regenerated contract doc and the generated copies.
- **Delivery:** a scoped commit, then `plan-ledger.js run <plan> --verify`, then `toolu-review:review` with v2 state, then `verdict.js status` reporting `overall: ready`, then push, a PR to `main`, and the `pr-babysit:babysit` handoff.
