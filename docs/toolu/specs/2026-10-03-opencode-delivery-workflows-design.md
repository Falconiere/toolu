# OpenCode: brainstorm and delivery-flow workflow semantics — Design

**Date:** 2026-10-03   **Status:** Draft   **Author:** Cursor agent (epic worker, #355)   **Topic:** Make the generated `brainstorm-brainstorm` and `delivery-flow-delivery-flow` skills, and every reference they link, name only OpenCode tools, skills, agents and paths, and prove the spec → plan → ledger → push approval transitions in real isolated repositories (OP-21)

## Problem

The substrate (#337–#345) and the core-workflow port (#358) let OpenCode discover both skills, but the generated text is a near-verbatim copy of the Claude Code / Codex sources. Reading `tools/toolu-opencode/generated/skills/{brainstorm-brainstorm,delivery-flow-delivery-flow}/` on `origin/main` (525652f5):

- **Locating toolu.** delivery-flow tells the model to find `TOOLU_PLUGIN_ROOT` via `installPath` in `claude plugin list --json` or `source.path` in `codex plugin list --json`. Neither CLI exists on OpenCode. toolu's `shell.env` already sets `TOOLU_PLUGIN_ROOT` to the enabled toolu plugin's directory in every bash call (`tools/toolu-opencode/src/host/runtime-env.ts`).
- **Workflow invocations.** `pr-babysit:babysit` (5 places) and `toolu-review:review` (2 places) stay in Claude/Codex syntax; the OpenCode ids are `pr-babysit-babysit-73c340c6` and `toolu-review-review`. The description ends "on Claude Code or Codex".
- **Delegation and models.** `execution.md` says to delegate to `toolu:quick-task` / `toolu:implementer` / `toolu:architect` "or an explicit `model:`". `ledger.md` documents step `model` values `haiku|sonnet|opus|fable|inherit` with no OpenCode meaning. OpenCode's `task` takes `subagent_type` and no model argument; an agent's model is `agent.<id>.model`.
- **Unported shared copies.** delivery-flow's private `model-routing.md` and `semantic-judgments.md` are byte-identical to toolu's, which #358 ported, but the port table is keyed by toolu's paths, so delivery-flow's copies still say Luna/Terra/Sol, `toolu:` agent ids, "pass `model:`", `$toolu:setup` and "the installed `jev` skill". Its `host-mapping.md` has no OpenCode column.
- **Questions.** brainstorm names `AskUserQuestion` (Claude Code) and `request_user_input` (Codex); OpenCode's tool is `question`.
- **Other host names.** `execution.md` names a "PostToolUse quality gate" and a nonexistent `systematic-debugging` skill.
- **Core diagnostics.** The native plan-ledger CLI, run from OpenCode's bash, refuses a Draft plan or spec with "run /delivery-flow:delivery-flow (… review phase)". toolu SessionStart's once-only migration notice tells OpenCode to "Install with `/plugin install delivery-flow@toolu`". Both reach the OpenCode model with Claude-only syntax.

## Non-Goals

1. Porting pr-babysit's own skill and fixer dispatch (OP-23, #357) or epic-orchestrator (OP-22). This issue only fixes how delivery-flow names and loads them.
2. Changing Claude Code or Codex output. Source skills keep their text, except delivery-flow's `host-mapping.md`, which gains the same OpenCode column as toolu's copy while its Claude Code and Codex cells stay byte-identical. The preflight and migration texts change only when the host is `opencode`.
3. Changing the plan-ledger schema, the spec/plan `Status` contract or any gate's logic. OpenCode is proven against the existing gates.
4. Choosing provider models for OpenCode agents (`agent.<id>.model` stays the only route, as in #358).
5. The CLI installer (OP-26), npm packaging completeness (OP-27) and mandatory CI acceptance (OP-28).

## Architecture

Decisions checked with Jev: extend #358's exact-anchor port table rather than editing sources or adding a generic rewriter (choice 1.00); keep the ledger `model` field and give its aliases an OpenCode meaning, a toolu agent passed as `subagent_type` (0.93); make the two core diagnostics OpenCode-aware (noul 0.77).

1. **Port table entries** in `tools/toolu-opencode/scripts/lib/opencode-port.ts`, keyed by repo-relative source path, for:
   - `plugins/brainstorm/skills/brainstorm/SKILL.md`: `question` replaces the two host tools; the optional broad-map delegation names `toolu-deep-explore` via `task`; Jev is loaded with `skill({ name: "jev-jev" })`.
   - `plugins/delivery-flow/skills/delivery-flow/SKILL.md`: description says OpenCode; the locate-toolu paragraph uses `shell.env`'s `TOOLU_PLUGIN_ROOT` and stops when it is unset; brainstorm, review and babysit are loaded with `skill({ name })` by their generated ids; the blockers list names those skill ids.
   - `references/execution.md`: tier → agent mapping for delegation; review and babysit by skill id; the post-edit check described as toolu's `tool.execute.after` check that appends the violation and blocks the next commit/push; `systematic-debugging` → `skill({ name: "toolu-debug" })`.
   - `references/ledger.md`: the `model` aliases are tier labels on OpenCode (`haiku` → `toolu-quick-task`, `sonnet` → `toolu-implementer`, `opus`/`fable` → `toolu-architect`, `inherit` → `general`), passed as `subagent_type`, never a model argument.
   - `references/model-routing.md` and `references/semantic-judgments.md`: the same edit lists as toolu's identical sources (shared constants, so the two copies cannot drift apart in OpenCode).
2. **Host mapping.** `plugins/delivery-flow/skills/delivery-flow/references/host-mapping.md` gets toolu's OpenCode column verbatim. It is a self-contained copy (no link out of the plugin).
3. **Core diagnostics.**
   - `packages/toolu-core/src/ledger/ledger-commands.ts`: `preflightChecks` takes the resolved host; on `opencode` the remedy reads `load skill({ name: "delivery-flow-delivery-flow" }) (plan review phase)` / `(spec review phase)`. The host comes from `resolveHost` over the CLI's env (`TOOLU_HOST_OVERRIDE=opencode` in OpenCode bash). Rebuilt into `plugins/toolu/hooks/dist/plan-ledger.js`.
   - `plugins/toolu/hooks/src/lifecycle/session-notices.ts`: `deliveryFlowNotice` gets an `opencode` branch: add `delivery-flow` to `enabled` in `.opencode/toolu/plugins.json`, then load `skill({ name: "delivery-flow-delivery-flow" })`. Rebuilt into `hooks/dist/session-start.js`.
4. **Regenerate** `tools/toolu-opencode/generated/` with the surface generator.

Reused: `applyPort`/`planSkillResources` (ports copied references by real path), `createTooluHooks`, `shell.env`, the native plan-ledger push gate, `plan-ledger.js`/`verdict.js` bundles, the published `toolu-review/write-state.sh`, `@toolu/conformance` sandboxes, and the pinned-host harness (`openSession`, `runHost`, scripted provider).

## Interfaces / Schema

- `OPENCODE_PORTS` gains six keys: brainstorm `SKILL.md`, delivery-flow `SKILL.md`, `references/execution.md`, `references/ledger.md`, `references/model-routing.md`, `references/semantic-judgments.md`. The toolu `model-routing.md` and `semantic-judgments.md` edit lists move to named constants that both keys reference.
- Generated ids named in the port: `brainstorm-brainstorm`, `delivery-flow-delivery-flow`, `toolu-review-review`, `pr-babysit-babysit-73c340c6`, `jev-jev`, `toolu-debug`, agents `toolu-quick-task`, `toolu-deep-explore`, `toolu-implementer`, `toolu-architect`, built-in `general`.
- Preflight stderr on OpenCode (exit 1):
  - `preflight: plan not approved (Status: Draft) — load skill({ name: "delivery-flow-delivery-flow" }) (plan review phase)`
  - `preflight: spec <path> not approved (Status: Draft) — load skill({ name: "delivery-flow-delivery-flow" }) (spec review phase)`
  - Claude Code and Codex keep `run /delivery-flow:delivery-flow (…)`.
- Migration notice on OpenCode: ``WARN: toolu workflow skills moved to delivery-flow (brainstorm, spec, spec-review, plan, plan-review, execution, test). Add `delivery-flow` to `enabled` in `.opencode/toolu/plugins.json`, then load `skill({ name: "delivery-flow-delivery-flow" })`.``
- Ledger state on OpenCode: `<repo>/.opencode/tmp/plan-ledger/<branch-slug>.json` (unchanged core path resolution).

## Failure modes and edge cases

- **Source drifts under a port anchor**, or an anchor repeats: generation throws naming the source path and anchor (existing `applyEdit`); `check:opencode-surface` fails instead of shipping Claude text.
- **A named skill id stops existing** (e.g. pr-babysit's hashed id changes when OP-23 renames its surfaces): the surface audit fails because every `skill({ name })` must name a generated skill directory.
- **`TOOLU_PLUGIN_ROOT` unset** (toolu not ready in this session): the generated skill says to stop and name that prerequisite; it never guesses a path.
- **Plan or spec not Approved / missing `Status`:** preflight exits 1 with the OpenCode remedy; a missing plan exits 2. No ledger write.
- **Push with ledger steps green but not verified, or stale after an edit:** the plan-ledger gate (`planLedger.mode: block`) denies the push before it runs; the remote is unchanged. After `run <plan> --verify` the push is allowed.
- **No ledger file for the branch:** the gate allows (existing behavior); delivery-flow's own `--verify` step is what creates it.
- **`question` tool absent:** brainstorm falls back to one concise plain question.
- **A step tier with no fitting OpenCode agent** (`fable`): mapped to `toolu-architect`; `inherit` maps to `general`, which runs the session model.
- **Host unknown to the ledger CLI** (plain shell): Claude Code text, as today.

## Acceptance criteria

- **AC-1:** With `brainstorm` and `delivery-flow` selected in a sandbox project, toolu's `config` hook registers `brainstorm-brainstorm` and `delivery-flow-delivery-flow` (plus their dependencies' skills). Every relative Markdown link reachable from both generated `SKILL.md` files resolves to a file in `generated/`, and both skills' frontmatter is host-valid (name equals directory, passes the name rule, description ≤ 1024 characters).
- **AC-2:** Across that link closure (excluding `host-mapping.md`), none of these appears: `AskUserQuestion`, `request_user_input`, `spawn_agent`, `EnterWorktree`, `CODEX_HOME`, `claude plugin`, `codex plugin`, `installPath`, `toolu:`, `$toolu:`, `pr-babysit:babysit`, `toolu-review:review`, `brainstorm:brainstorm`, `Pass \`model:\``, `explicit \`model:\``, `Claude Code or Codex`, `PostToolUse`, `systematic-debugging`. Every `skill({ name })` names a generated skill, every `toolu-*` id named is a generated agent or skill, and every `$TOOLU_PLUGIN_ROOT/…` path exists in a `stagePlugins` copy of toolu.
- **AC-3:** The generated `ledger.md` and `execution.md` state the OpenCode meaning of each step `model` alias (`toolu-quick-task`, `toolu-implementer`, `toolu-architect`, `general` via `subagent_type`) and that `task` takes no model argument; the generated `model-routing.md` matches the generated toolu orchestrator copy.
- **AC-4:** The generated delivery-flow `host-mapping.md` has the OpenCode column, and its Claude Code and Codex cells are byte-identical to `origin/main`'s delivery-flow copy.
- **AC-5:** Generation fails naming the source and anchor when a delivery-flow port anchor is removed or duplicated.
- **AC-6:** In a real isolated git repository with a bare remote, through `createTooluHooks` with `planLedger.mode: block`, running the generated skill's commands in the `shell.env` bash: `plan-ledger.js preflight` exits 1 naming `skill({ name: "delivery-flow-delivery-flow" })` for a Draft spec and for a Draft plan, and exits 0 when both are Approved; `run <plan> --step <id>` writes `.opencode/tmp/plan-ledger/<slug>.json`; `verdict.js status` reports `overall: blocked`; `git push` is denied by the plan-ledger gate and the remote is unchanged; after `run <plan> --verify` and the review write-state command, `verdict.js status` reports `overall: ready`, the push is allowed and the remote has the commit.
- **AC-7:** `plan-ledger.js preflight` with no OpenCode host keeps the `run /delivery-flow:delivery-flow (…)` text; toolu SessionStart on OpenCode emits the OpenCode migration notice, and on Claude Code and Codex the existing lifecycle goldens pass unchanged.
- **AC-8:** With `TOOLU_LIVE_OPENCODE=1`, on the pinned host with a scripted provider: the model loads `delivery-flow-delivery-flow` and `brainstorm-brainstorm` through the native `skill` tool, reads a delivery-flow reference through `read`, sees preflight fail on the Draft plan and pass on the Approved one, runs `run <plan> --verify`, and pushes; the remote has the commit.
- **AC-9:** Claude Code and Codex are intact: among plugin sources only delivery-flow's `host-mapping.md` (column added), `ledger-commands.ts` and `session-notices.ts` change, and `bun run test` passes.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Committed `generated/`; `createTooluHooks` `config` hook in a sandbox selecting `brainstorm`, `delivery-flow` | Both skills registered; closure resolves; frontmatter valid | Dependencies closed | `bun test tools/toolu-opencode/scripts/__tests__/delivery-surfaces.test.ts` and `bun test tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.test.ts` |
| AC-2 | Same closure; `stagePlugins` into a sandbox | No banned token; ids and paths resolve | `host-mapping.md` excluded from the token scan only | `delivery-surfaces.test.ts` |
| AC-3 | Generated `ledger.md`, `execution.md`, `model-routing.md` | Mapping sentences present; routing copies equal | `fable`, `inherit` | `delivery-surfaces.test.ts` |
| AC-4 | Generated host mapping; `git show origin/main:plugins/delivery-flow/…/host-mapping.md` | Column present; other cells equal | Row count unchanged | `delivery-surfaces.test.ts` |
| AC-5 | Sandbox copy of `plugins/delivery-flow` with an anchor removed, and duplicated | `planSurface` throws naming path and anchor | 0 and 2 matches | `delivery-surfaces.test.ts` |
| AC-6 | Sandbox repo, bare remote, Draft/Approved spec and plan docs, a real `bun test` file | Transitions and outputs as stated | Draft spec vs Draft plan; unverified green | `delivery-workflows.test.ts` |
| AC-7 | `plan-ledger.js preflight` without the override; `session-start.js` under `opencode`, `claude`, `codex` | Old text off OpenCode; OpenCode notice; goldens unchanged | Notice is once-only | `bun test packages/toolu-core/src/ledger/__tests__/` and `bun test plugins/toolu/hooks/src/__tests__/` |
| AC-8 | Pinned `opencode-ai@1.18.34`, scripted provider | Tool states and remote as stated | Live only | `TOOLU_LIVE_OPENCODE=1 bun test tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.live.test.ts` |
| AC-9 | Source tree vs `origin/main` | Only the named sources change | — | `git diff --stat origin/main -- plugins/*/skills plugins/*/agents plugins/*/commands` and `bun run test` |

## Documentation impact

- `docs/opencode.md`: a "Delivery workflows" subsection: skill ids, `TOOLU_PLUGIN_ROOT`, tier → agent mapping, ledger state path, preflight remedy text.
- `plugins/delivery-flow/README.md` and `plugins/brainstorm/README.md`: an OpenCode invocation note (`skill({ name })`, enabling in `plugins.json`).
- Regenerated copies under `generated/resources/` and the generated notes.

## Open Questions

- None blocking. pr-babysit's hashed skill id is owned by OP-23; if that work renames it, the surface audit (AC-2) fails and the port entry is updated there.
