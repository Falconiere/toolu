# epic-creator plugin — Design

**Date:** 2026-11-01   **Status:** Approved   **Author:** Falconiere Barbosa (agent)   **Topic:** A skill that turns an idea into a GitHub epic the epic engine can run, with token-lean sub-issue bodies

Brainstorm record: `docs/toolu/brainstorms/2026-11-01-epic-creator.md` (Full path, Jev-backed).

## Problem

Epics are written by hand today. Two review passes (#649, #660) found
sub-issue specs that contradicted the code, so every sub-issue needed a human
"Review additions" section before workers could build. #660's epic body
(15.6k chars) was skimmed by all nine workers, and its background link pointed
at a gitignored file that 404s on GitHub. Each sub-issue body is the prompt an
unattended worker re-reads on every turn (one #737 worker reached 167M input
tokens), and padding raises its routing tier (`route/tier.rs`: excerpt over
1200 chars or six checkboxes means tier 2). There is no skill that produces an
epic in the exact shape the engine reads, with bodies that are both correct and
small.

## Non-Goals

1. Jira and Linear writes. The skill names them as not supported yet and stops.
2. A native `toolu epic create` verb. Writes stay agent-driven `gh api` calls.
3. Editing, closing or re-parenting existing issues.
4. Starting the epic engine, launching workers, editing code or committing.
5. A task-list mirror of the sub-issues in the epic body.

## Architecture

A new Markdown-only plugin `plugins/epic-creator`, shaped like
`plugins/brainstorm`: three host manifests, a README, one skill and two
references, plus a guide-only crate `crates/epic-creator` (`toolu
epic-creator` prints a guide, no verbs). The plugin depends on `brainstorm`
(as delivery-flow does) and runs it first; brainstorm's Delivery mode is
generalized from "delivery-flow" to "a calling skill", which then hands off to
the caller's next phase.

Skill phases: preflight → think (brainstorm) → decompose under eight body
rules → one confirmation → create via `gh api` → prove with
`toolu --json epic graph <epic> --dry-run` and `toolu epic route --dry-run`.

The decisive trade-off: agent-driven writes cost no Rust and ship now; the
writes live in one reference so a native verb can replace them when a second
tracker is requested (Jev 0.81 vs 0.05 for a native verb now).

Reused paths: `crates/brainstorm` (crate template),
`crates/epic-orchestrator/src/tracker/github_fetch.rs` (reader contract),
`crates/epic-orchestrator/src/route/tier.rs` (routing heuristic). The
text-blocker regex in `text_blockers` is extracted into a pure function so a
unit test can run the real reader on the shipped example body without a
network client.

## Interfaces / Schema

**Skill:** `/epic-creator:epic-creator <idea> [--repo owner/name] [--dry-run]`
(Codex `$epic-creator:epic-creator`). `<idea>` is free text or a path to a
brainstorm or spec file. `--dry-run` stops after the draft.

**Files:**

```text
plugins/epic-creator/
  .claude-plugin/plugin.json   name, version 8.8.0, hookProtocol 2, dependencies [brainstorm]
  .codex-plugin/plugin.json    same name/version/description, skills ./skills/, dependencies
  .muse-plugin/plugin.json     schemaVersion 1, one skill capability
  README.md
  skills/epic-creator/SKILL.md
  skills/epic-creator/references/issue-bodies.md   epic + sub-issue templates, one filled example
  skills/epic-creator/references/github-writes.md  ordered gh api calls, failure handling
crates/epic-creator/  Cargo.toml, src/lib.rs (PLUGIN, command, run), src/tests/lib_test.rs
```

**GitHub writes (REST, verified via Context7 2026-11-01):**

| Step | Call | Body |
| --- | --- | --- |
| label | `gh label create epic` only when `gh label list` lacks it | — |
| epic | `POST /repos/{o}/{r}/issues` | `title`, `body`, `labels: ["epic"]` |
| child | `POST /repos/{o}/{r}/issues` | `title`, `body` |
| link | `POST /repos/{o}/{r}/issues/{epic}/sub_issues` | `sub_issue_id` (issue **id**) |
| dep | `POST /repos/{o}/{r}/issues/{child}/dependencies/blocked_by` | `issue_id` (blocker **id**) |

**Body rules the skill enforces:** cite or omit (`path:line` at a pinned
`Base: <sha>`), decisions not research, one runnable check per AC, front-loaded
and under 1500 chars (ceiling 3000), at most five AC checkboxes, other issues
named only under `## Relationships`, links must be tracked on the base branch,
epic body under 4000 chars as a contract index, each child sized to the
"Standard" tier.

**Code change:** `github_fetch.rs` gains
`pub(super) fn text_blocker_refs(body: &str, repo: &str, own: &str) -> Result<Vec<String>, String>`;
`text_blockers` calls it. Behavior is unchanged.

## Failure modes and edge cases

- **gh unauthenticated or repo unreadable:** preflight stops with the failing
  command and its fix; nothing is created.
- **Jira or Linear asked:** the skill says GitHub only for now and stops.
- **User declines or revises at confirmation:** no write happens; revise loops
  back to decompose once per request.
- **A POST fails mid-way:** the skill stops, never retries a creation POST
  blindly (a lost response may have committed), lists created issue numbers and
  missing links/deps, and checks with a GET before any retry.
- **Missing `epic` label:** created first; a create failure stops before the
  epic.
- **Body over 3000 chars, six or more checkboxes, a foreign issue reference
  outside `## Relationships`, or an untracked link:** the self-check flags it
  and the draft is fixed before confirmation.
- **A dependency cycle in the draft:** detected before confirmation; the user
  picks the wrong edge.
- **Proof mismatch** (graph lacks a child, edges differ, cycle, tier above 1):
  reported with the exact difference; fixes to just-created issues go through
  `gh issue edit`, nothing else is touched.
- **Secondary rate limit:** writes are sequential, one call each.

## Acceptance criteria

- **AC-1:** Given the shipped filled example sub-issue body, the real GitHub
  reader's text-blocker extraction yields exactly the issue named under
  `## Relationships` and nothing else; the epic template yields no edges.
- **AC-2:** Given the shipped filled example sub-issue body, the real routing
  heuristic returns tier 1 (Standard): excerpt under 1200 chars and fewer than
  six checkboxes.
- **AC-3:** `toolu epic-creator` prints its guide naming
  `/epic-creator:epic-creator`, exits 0 with empty stderr, and `--json` output
  validates as a `guide` document.
- **AC-4:** The plugin is a first-class catalog member: both marketplaces,
  three manifests with identical name/version/description, the CLI tree,
  release-please extra-files, folders.json, and inventory counts all include
  it, and the inventory and packaging checks pass.
- **AC-5:** The shipped skill body is under 150 lines, its description stays in
  its context budget with trigger phrases "create an epic" and "break this into
  issues", requires one confirmation before the first write, names both GitHub
  link endpoints, and ends with the graph and route dry-runs.
- **AC-6:** Every command in the skill and references passes
  `cargo xtask check-markdown-cli`.
- **AC-7:** brainstorm's Delivery mode names a calling skill generically, and
  delivery-flow's contract tests still pass.
- **AC-8:** `cargo xtask gate` passes on the branch.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
| --- | --- | --- | --- | --- |
| AC-1 | `issue-bodies.md` example and epic template, `include_str!` | `["o/r#12"]`; epic `[]` | prose mentioning `#7` outside Relationships is absent; own ref skipped | `cargo test -p toolu-epic-orchestrator epic_creator` |
| AC-2 | same example through `excerpt` + `heuristic_tier` | `1` | chars < 1200, checkboxes < 6 asserted separately | same |
| AC-3 | built binary | guide text, exit 0, schema valid | `--json` | `cargo test -p toolu-epic-creator`, `cargo test -p toolu-cli --test contract` |
| AC-4 | repository manifests | counts 17 plugins / 21 skills | name mismatch would fail inventory | `cargo test -p toolu-cli --test contract`, `cargo test -p toolu-plugins catalog`, `cargo xtask gate` |
| AC-5 | shipped SKILL.md | structural asserts hold | line count < 150 | `cargo test -p toolu-cli --test epic_creator_skill`, `cargo xtask context-budget` |
| AC-6 | skill + references | no findings | — | `cargo xtask check-markdown-cli` |
| AC-7 | brainstorm SKILL.md | wording generic | — | `cargo test -p xtask --test skills_contract` |
| AC-8 | branch | green | — | `cargo xtask gate` |

Live creation of a real epic is not part of the automated evidence: it is an
outward-facing write in a real repository. The reader contract it depends on
is proven by AC-1/AC-2 against the real code.

## Documentation impact

- New `plugins/epic-creator/README.md`.
- Root `README.md` plugin table and tree, `docs/plugins/index.md`.
- `docs/cli/` regenerated by `cargo xtask docs-cli` (new `epic-creator.md`,
  README row, `commands.json`).
- `docs/epic-orchestrator/README.md`: one line pointing to epic-creator for
  writing an epic.
- `plugins/brainstorm/skills/brainstorm/SKILL.md` mode wording.

## Open Questions

None blocking. Owner: user. Whether to add a native write verb when Jira or
Linear is requested is deferred by Non-Goal 2.
