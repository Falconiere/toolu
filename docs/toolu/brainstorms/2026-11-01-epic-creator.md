# epic-creator: from an idea to a runnable GitHub epic

**Outcome:** A new Markdown-only plugin `epic-creator` whose skill
(`/epic-creator:epic-creator <idea>`) thinks a change through like brainstorm,
decomposes it into one-PR sub-issues with acceptance criteria, test scenarios
and dependencies, shows the full draft once for confirmation, then creates the
GitHub epic issue, its native sub-issues and native `blocked_by` dependencies
in the exact shape the epic-orchestrator engine reads. It ends by proving the
shape with `toolu --json epic graph <epic> --dry-run` and `toolu epic route
--dry-run`, and suggests `/epic-orchestrator:epic-orchestrator <epic>` next.

**Material defaults / non-goal:** GitHub only in this slice; Jira and Linear
are named as later trackers, with the tracker writes isolated in one reference
file so a later native `toolu epic create` verb can replace them. The plugin
runs `gh` directly (already on the markdown-cli external list) and adds no new
Rust beyond a guide-only crate copied from brainstorm. Standalone only: it never
edits code, never starts the engine, never runs delivery-flow. Non-goals:
editing existing epics, Jira/Linear writes, a native write verb, a task-list
mirror in the epic body, auto-launching workers.

**Repository evidence:** `crates/epic-orchestrator/src/tracker/github_fetch.rs`
reads native sub-issues (fallback: task-list links), native `blocked_by`
(fallback: "blocked by" text anywhere in a line), labels and a body excerpt;
`route/tier.rs` scores title, labels, checkbox count and excerpt length over
1200 chars. Real epic #660 and sub-issue #662 fix the body conventions:
label `epic`, title `Epic: …`, background link to a brainstorm file, scope and
non-goals; children start with `# Part of #660`, then `## Acceptance criteria`
and `## Test scenarios` checklists and `## Relationships` with `Blocked by
#661`, label `enhancement`. `plugins/brainstorm` plus `crates/brainstorm`
(31-line guide crate) is the Markdown-only plugin precedent; delivery-flow
shows how a skill chains `brainstorm:brainstorm` as phase 1. The `toolu` binary
is monolithic, so `toolu epic graph --dry-run` is available wherever toolu is
installed and makes no tracker mutation. GitHub REST (Context7, 2026-11-01):
`POST /repos/{o}/{r}/issues/{n}/sub_issues {sub_issue_id}` and
`POST /repos/{o}/{r}/issues/{n}/dependencies/blocked_by {issue_id}`, both
taking issue ids, not numbers.

**Jev (2026-11-01):** home `markdown_plugin` 0.81 (native verb 0.05, skill
inside epic-orchestrator 0.14); confirm-before-create 0.88; graph dry-run as
the only proof 0.48, so the skill also runs `route --dry-run` and a body
self-check before declaring the epic ready.

**Risk:** Agent-driven `gh` loops can leave a half-created epic on a failure
mid-way; the skill must create the epic first, then children, then links, and
on any failure report exactly which issues exist and which links are missing so
the user can finish or close them. Prose that mentions another issue outside
`## Relationships` creates a false dependency edge (memory:
epic-tracker-edits-authorized). Sub-issue sizing is a judgment; oversized
issues surface as high tiers in `route --dry-run`. A missing `epic` label makes
`gh issue create` fail; the skill checks labels first. Secondary rate limits
on fast creation; the skill creates sequentially.

**Handoff:** `/delivery-flow:delivery-flow` to build it, with this file as the
brainstorm record.

## Axes

| Axis | Default | Evidence | Risk |
| --- | --- | --- | --- |
| Intent | Idea → confirmed draft → GitHub epic + sub-issues + deps → dry-run proof | User request; #660/#662 shape; `github_fetch.rs` | Draft quality depends on the agent's decomposition |
| Interface | New plugin `epic-creator`, skill `/epic-creator:epic-creator <idea>`; guide namespace `toolu epic-creator` with no verbs | brainstorm precedent; branch name `epic-creator`; Jev 0.81 | Extra registration points (registry, catalog tests, tree test, budgets, marketplace, folders.json, docs-cli) |
| Dependencies | Marketplace dependency on `brainstorm` only; `gh` and `toolu` binaries at runtime | delivery-flow declares brainstorm the same way | None material |
| Tracker writes | Sequential `gh api` calls documented in `references/github-writes.md`: create epic, create children with `# Part of #E`, add sub-issue links, add blocked_by | Context7 endpoint shapes | Half-created epic on failure; secondary rate limit |
| Issue bodies | Templates in `references/issue-templates.md` mirroring #660/#662; other issues named only under `## Relationships` | `github_fetch.rs` text-blocker regex; memory | A stray "blocked by" phrase forges an edge |
| Confirmation | One structured question with the full draft before any write | Jev 0.88; outward-facing mutation | None |
| Proof | `toolu --json epic graph <epic> --dry-run` then `toolu epic route --graph … --dry-run`; report waves, blockers, cycle, tiers | Both verbs exist and mutate nothing; Jev 0.48 for graph alone | Needs `gh auth` and core rate limit headroom |
| Horizon | GitHub now; Jira/Linear later behind the same skill via a native `toolu epic create --tracker` verb when they land | Tracker readers already exist in Rust | Skill rewrite of the writes section when the verb lands |

## Alternatives rejected

- **Native `toolu epic create --plan plan.json` now.** Deterministic writes and a
  direct path to Jira/Linear through the existing tracker clients, but roughly
  200+ lines of Rust plus fixtures for a GitHub-only slice. Jev 0.05. Revisit
  when a second tracker is requested.
- **Second skill inside `epic-orchestrator`.** No new plugin, but it drags the
  delivery-flow, toolu, toolu-review and pr-babysit dependency chain into a
  planning-only tool and blurs the "engine merges, you do not" boundary of that
  plugin. Jev 0.14.
- **Mirror a task list in the epic body.** Redundant with native sub-issues
  (the reader prefers native) and drifts when issues are added later.
- **Auto-start the engine after creation.** Different authorization scope;
  the user already has `/epic-orchestrator:epic-orchestrator` for that.

## Prompt design: the sub-issue is the worker's prompt (added 2026-11-01)

**What a worker pays for.** The brief (about 7 KB, fixed) sends the worker to
`gh issue view <url> --comments`, a skim of the epic body, closed blockers'
PRs, `AGENTS.md`, then delivery-flow's six phases. Cost is roughly turns times
context; cached input still bills per turn. Epic #737's codex worker on #739
reached 167M input tokens (166M cached) and 231k output tokens while the run
spent 34% of its time in pr-open and 41.5% in ready. The engine's waiting
prompts are its problem; the context each prompt re-reads is ours.

**Where past epics burned.** Two review passes (#649, #660, comemory
1b448e96 and c04e790e) found sub-issue specs that contradicted the code: wrong
field names, a forbidden plugin-to-plugin crate dependency, assumed code paths.
Every sub-issue needed a human "Review additions" section before workers could
build. #660's epic body (15.6k chars) was skimmed by all nine workers and its
"Background" link pointed at `docs/toolu/brainstorms/…`, which is gitignored
and returns 404 on GitHub: nine workers read a pointer to nothing. Sub-issue
#662 opened with `# Part of #660` plus a repeated title, spending the first
lines of the 1500-char routing excerpt on boilerplate. `route/tier.rs` raises
the heuristic tier when the excerpt exceeds 1200 chars or holds six or more
checkboxes, so padding routes an issue to a bigger model.

**Jev (2026-11-01):** pre-decided contract over goal-plus-ACs 0.85; compact
epic body with decisions and links over inline contracts 0.92; body near the
1500-char excerpt, decisive content first, 0.67 (about 3000 chars 0.32);
mandatory separate claim-verification pass 0.55, so verification is folded
into drafting as "cite or omit" instead of a second pass.

### Rules the skill enforces on every body

1. **Cite or omit.** A file, symbol, field, constraint or existing behavior
   appears in a body only with a `path:line` the skill read in the checkout at
   the pinned base commit (`Base: <sha>` line). No citation, no claim. This is
   the cheapest fix for the dominant rework cause and costs nothing extra: the
   skill already had to look to know.
2. **Decisions, not research.** The body states what was decided (approach,
   interface or schema shape, files to touch, non-goals, failure behavior),
   never the alternatives or the reasoning; those stay in the local brainstorm
   record. A worker's brainstorm phase then takes the Minimal path.
3. **One runnable check per AC.** Each acceptance criterion names the command
   or test that proves it, against a real input, so the worker's spec and plan
   are transcription and the ledger's `ac_refs` map one to one.
4. **Front-loaded and small.** Order: scope line, decisions, interfaces, files,
   ACs, test scenarios, non-goals, relationships. Target under 1500 chars; hard
   ceiling 3000, and only when an interface shape needs the room. No `# Part
   of` header and no repeated title: the native parent link carries that.
5. **Other issues only under `## Relationships`.** `Blocked by #N` is the
   only place another issue is named; the reader treats "blocked by" or
   "depends on" anywhere in a line as an edge.
6. **Links must resolve.** A linked repository file passes
   `git ls-files --error-unmatch <path>` on the base branch, else it is inlined
   or dropped. Nothing under an ignored `docs/toolu/` is linked.
7. **Epic body is a contract index.** Goal in two sentences, agreed decisions
   as one-line bullets, shared conventions as a pointer to `AGENTS.md`,
   non-goals, the base commit, and nothing else. Under 4000 chars.
8. **Sizing by the tier scale.** Each sub-issue should land at Standard on the
   engine's tier scale (one module, existing patterns, clear ACs). A draft that
   reads Complex is split along a native dependency, not padded with caveats.
   `toolu epic route --dry-run` is the check: a tier-2 or tier-3 issue is
   re-examined before confirmation.

### What the creator itself must not spend

- No separate verification pass and no delegated re-exploration: the skill
  reads each file once while drafting and cites from that read.
- One structured confirmation with the full draft; no per-issue questions.
- One `gh api` call per issue, link and dependency, sequential; no polling.
- The dry-run graph and route are run once each, after all writes.
