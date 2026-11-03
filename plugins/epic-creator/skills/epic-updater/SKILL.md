---
name: epic-updater
description: Refresh an existing GitHub epic or proposal against current code, decisions and delivery evidence. Find gaps and stale requirements, then update descriptions, acceptance criteria and test plans. Use dry-run for an audit only.
---

# Epic updater

Input: a GitHub issue URL, `owner/repo#N` or `#N` in the target checkout.
`--repo owner/name` must match that checkout; `--dry-run` audits and drafts
without GitHub writes. Jira and Linear are not supported.

Invoking this skill to update authorizes body edits to the named issue and
its open, same-repository children within the existing goal. An audit or
dry run authorizes reads and local drafts only. Prepare the exact diff before
writing; do not ask again for already-authorized body edits. New scope,
new issues, relationship changes, titles, closure or reopening need explicit
authorization for the concrete proposed change. Never start or pause the
engine, change code, commit, or send worker messages through this skill.

## 1. Snapshot

Use the creator's [preflight](../epic-creator/SKILL.md#1-preflight) to resolve
`REPO`, `BASE`, `SHA` and scratch `DIR`; do not run its creation workflow.
Resolve the complete input reference and require its repository to match
`REPO` before taking its number as `EPIC`; reject PR URLs and repo mismatches.
Record the full parent body, state,
`updated_at`, comments and child links.
Read the engine's view once:

```bash
gh api "repos/$REPO/issues/$EPIC" > "$DIR/parent.json"
gh api --paginate --slurp "repos/$REPO/issues/$EPIC/comments?per_page=100" > "$DIR/comments.json"
gh api --paginate --slurp "repos/$REPO/issues/$EPIC/sub_issues?per_page=100" > "$DIR/children.json"
toolu --json epic graph "$REPO#$EPIC" --dry-run > "$DIR/before-graph.json"
toolu epic status --fleet
```

Use the graph's children, including legacy task-list children when native
links are absent. Fetch each child's full body,
comments and paginated native dependencies; graph excerpts are not complete
issue bodies. The slurped listings contain an array of pages. Inspect linked
PR status and merged changes relevant to its
criteria. Cache these reads; refresh only for new evidence or before a write.
Cross-repository children are read-only findings requiring their own checkout.
An inaccessible issue or failed page is missing evidence, never an empty list.

No children means a proposal, not a completed epic. Audit its body normally;
offer a concrete decomposition if useful, but never invent children or call it
ready for orchestration. Closed issues are evidence, not rewrite targets.

Check status and linked open PRs for work in progress. If the epic is running,
its state is unknown, or a target child has active work, draft the affected
changes but defer those writes until the operator establishes a safe handoff.
A paused engine alone does not prove its workers have finished their step.
Continue the audit; do not silently change a worker's contract.

## 2. Reconcile

Read the shared [evidence workflow](../epic-creator/references/evidence.md).
Compare each existing requirement against the pinned base, later decisions,
merged work and tests. Classify it as current, stale, missing, implemented,
conflicting or unverified; attach evidence and the smallest proposed action.
An open PR is not delivered work. A failed search is not proof of absence.

Check the whole path from entry point through callers, persistence and failure
handling to documentation and verification. Look for duplicate work, missing
prerequisites and contradictory contracts across children. Reuse settled
decisions; brainstorm only a material unresolved choice, not the entire epic.
Use Jev for evidence-backed semantic judgments when available, then verify
paths, commands and dependency edges deterministically.

## 3. Draft

Use [issue bodies](../epic-creator/references/issue-bodies.md) for changed
sections. Preserve unrelated prose, user decisions, issue identity, completed
checkboxes and legacy task-list links. Do not replace an existing body just to
fit the creator template. Keep old evidence attributed to its original base;
only refreshed claims may cite the new `SHA`.

Rewrite stale descriptions into current behavior and intended behavior. Give
each changed criterion an observable input/result and a targeted check. Name
failure and compatibility cases where relevant. Mark new tests as planned;
do not tick criteria because the code looks plausible or a broad suite passed.
Keep completed criteria as history; put newly discovered residual work in a
proposed follow-up rather than reopening delivered scope implicitly.

Show a compact table: issue, finding, evidence, proposed edit, verification.
Save before/after bodies and show their diffs. Separate body edits from
structural proposals and active-work deferrals. A dry run stops here with the
draft paths. If nothing needs changing, report that and perform no writes.

## 4. Apply and prove

Follow [safe updates](references/github-updates.md) for authorized body edits.
If structural work is needed, prepare its exact bodies and edges first and
obtain only the missing authorization. For authorized additions, reuse the
creator's GitHub-write reference for issue creation and dependencies, never
recreate the existing parent. Preserve the parent's membership format: when
native children are absent and the graph uses legacy task-list children, add
the new child to that task list through the safe body-update procedure and
skip the native sub-issue link call. The first native link would hide all legacy
children from the engine. Use native links for already-native epics; converting
a legacy epic requires a separately authorized migration of every retained
child. Recheck membership before linking and defer if its format changed.

Read back every edited body and rebuild the graph:

```bash
toolu --json epic graph "$REPO#$EPIC" --dry-run > "$DIR/after-graph.json"
```

Compare child identities, states and blocker edges to the snapshot: body-only
updates must preserve them, and introduce no cycle. Route the edited children with
`toolu epic route --graph "$DIR/after-graph.json" --dry-run --no-jev`, repeating
`--issue "$REPO#$CHILD"` for each. Report routing changes; never delete useful
requirements to force a cheaper tier. For a childless proposal, skip routing
and say it still needs decomposition.

Report updated issue links, deferred/proposed work, checks actually run and
test plans still to implement. Include scratch paths for recovery. Do not
claim tests passed or the epic is execution-ready without observed evidence.
