---
name: epic-creator
description: Turn an idea into a GitHub epic the epic-orchestrator engine can run. Use to create an epic, plan work as an epic, or break this into issues. It drafts one-PR sub-issues with cited decisions and runnable checks, confirms once, then creates the epic, native sub-issues and dependencies. GitHub only for now.
---

# Epic creator

Each sub-issue body becomes the prompt an unattended worker re-reads on every
turn, and the text the engine routes on. Write bodies that let a worker build
the right thing with the least reading. This skill creates issues; it never
edits code, commits or starts the engine.
To refresh existing issues, use the sibling `epic-updater` skill instead.

## Inputs

`<idea>`: free text, or a path to a brainstorm or spec file. Options:
`--repo owner/name` (must be the checkout's own GitHub repository); `--dry-run`
drafts and stops before any write. Jira or Linear: say they are not supported
yet and stop.

## Authorization

After the user confirms the draft, this skill may create in the target
repository the `epic` label when missing, the epic, its children, their
sub-issue links and their dependencies, and fix issues this run created.
Brainstorm may write its local record under `docs/toolu/brainstorms/`; nothing
is committed. Nothing else: no edits to other issues, no code, no commits, no
engine.

## 1. Preflight

Stop with the fix for the first failure:

```bash
gh auth status
gh repo view --json nameWithOwner,defaultBranchRef --jq '.nameWithOwner + " " + .defaultBranchRef.name'
git fetch origin
git rev-parse --short "origin/$BASE"
```

The second line prints the checkout's repository and default branch: set
`REPO` and `BASE` from it. A `--repo` that differs from it stops the run:
citations must come from the target's own checkout, so run there. The last
line is the **base commit**, `SHA`. Every citation and every `Base:` line
refers to it; read code there (`git show "$SHA:<path>"`) when the checkout
differs. Make one scratch directory, `DIR=$(mktemp -d)`, for bodies and the
graph.

## 2. Think

Run `brainstorm:brainstorm` (`/brainstorm:brainstorm` in Claude Code,
`$brainstorm:brainstorm` in Codex) in its Delivery mode on the idea; it hands
back to step 3 instead of a spec. Keep its decisions, evidence and risks. When
the input is an approved spec, skip it and reuse the spec's decisions.

## 3. Decompose

Read [evidence and handoff](references/evidence.md) and
[issue bodies](references/issue-bodies.md), then draft the epic and every
child under their rules. While drafting:

- Read each file once and cite from that read. Leave out what you did not read.
- Make each child one coherent PR with observable criteria and focused checks.
  Account for callers, tests, fixtures and docs; prefer at most five criteria.
- Order children by dependency and confirm the draft has no cycle.
- State a shared decision once, in the epic. A child repeats only what it obeys.

Check every body before showing it; fix broken claims, links and relationships:

- target a child under 1200 characters and the epic under 4000; explain any
  larger body or six-plus criteria instead of dropping necessary requirements;
- no "blocked by" or "depends on" outside `## Relationships`, and no `#N` task lines in the epic;
- every linked file passes `git cat-file -e "$SHA:<path>"`;
- every `path:line` exists at the base commit.

Body length, task count and fan-out can raise heuristic routing cost; title
and labels also affect it. Keep real dependencies and complete contracts,
and show expected routing costs in the table rather than promising Standard.

## 4. Confirm

Show one table (child, title, blocked by, characters, expected tier/reason)
and the full bodies, then
ask one structured question: create, revise or cancel (`AskUserQuestion` in
Claude Code, `request_user_input` in Codex). Revise returns to step 3.
`--dry-run` and cancel stop here with nothing written.

## 5. Create

Follow [GitHub writes](references/github-writes.md): label, epic, children in
dependency order, sub-issue links, dependencies. On a failure, stop, never
repeat a create, and report what exists.

## 6. Prove

Read the epic back through the engine's own reader and router:

```bash
toolu --json epic graph "$REPO#$EPIC" --dry-run > "$DIR/graph.json"
toolu epic route --graph "$DIR/graph.json" --dry-run --no-jev --issue "$REPO#$CHILD"
```

Repeat `--issue` once per child. `--no-jev` keeps the check on the body shape;
the engine may still score with Jev when it runs. The epic is ready when the
graph lists every child, each child's `blockers` match the drafted edges and
`cycle` is empty. Explain unexpected routing tiers; remove repetition or split
at a coherent boundary only when the full contract survives. Fix a body with
`gh issue edit`; add a
missing link or dependency with the calls in GitHub writes. Removing a link or
dependency needs the user's approval. Then prove again.

## Report

Print the epic URL, one line per child (number, title, blockers, tier), and the
next step: `/epic-orchestrator:epic-orchestrator <epic> --dry-run` in Claude
Code or `$epic-orchestrator:epic-orchestrator <epic> --dry-run` in Codex.
