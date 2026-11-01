---
name: epic-creator
description: Turn an idea into a GitHub epic the epic-orchestrator engine can run. Use to create an epic, plan work as an epic, or break this into issues. It drafts one-PR sub-issues with cited decisions and runnable checks, confirms once, then creates the epic, native sub-issues and dependencies. GitHub only for now.
---

# Epic creator

Each sub-issue body becomes the prompt an unattended worker re-reads on every
turn, and the text the engine routes on. Write bodies that let a worker build
the right thing with the least reading. This skill creates issues; it never
edits code, commits or starts the engine.

## Inputs

`<idea>`: free text, or a path to a brainstorm or spec file. Options:
`--repo owner/name` (default: the checkout's GitHub repository); `--dry-run`
drafts and stops before any write. Jira or Linear: say they are not supported
yet and stop.

## Authorization

After the user confirms the draft, this skill may create in the target
repository the `epic` label when missing, the epic, its children, their
sub-issue links and their dependencies, and fix issues this run created.
Nothing else: no edits to other issues, no code, no commits, no engine.

## 1. Preflight

Stop with the fix for the first failure:

```bash
gh auth status
gh repo view "$REPO" --json nameWithOwner,defaultBranchRef --jq '.nameWithOwner + " " + .defaultBranchRef.name'
git fetch origin
git rev-parse --short "origin/$BASE"
```

The last line is the **base commit**. Every citation and every `Base:` line
refers to it; read code there (`git show "$SHA:<path>"`) when the checkout
differs.

## 2. Think

Run `brainstorm:brainstorm` (`/brainstorm:brainstorm` in Claude Code,
`$brainstorm:brainstorm` in Codex) in its Delivery mode on the idea; it hands
back to step 3 instead of a spec. Keep its decisions, evidence and risks. When
the input is an approved spec, skip it and reuse the spec's decisions.

## 3. Decompose

Read [issue bodies](references/issue-bodies.md) and draft the epic and every
child under its rules. While drafting:

- Read each file once and cite from that read. Leave out what you did not read.
- Make each child one PR: one module, at most five criteria, each with a check.
- Order children by dependency and confirm the draft has no cycle.
- State a shared decision once, in the epic. A child repeats only what it obeys.

Check every body before showing it, and fix rather than explain:

- under 1500 characters (3000 only for an interface shape); the epic under 4000;
- fewer than six `- [ ]` boxes;
- no other issue named, and no "blocked by" or "depends on", outside `## Relationships`;
- every linked file passes `git ls-files --error-unmatch <path>`;
- every `path:line` exists at the base commit.

## 4. Confirm

Show one table (child, title, blocked by, characters) and the full bodies, then
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
toolu epic route --graph "$DIR/graph.json" --dry-run --issue "$REPO#$CHILD"
```

Repeat `--issue` once per child. The epic is ready when the graph lists every
child, each child's `blockers` match the drafted edges, `cycle` is empty, and
no child routes above `standard`. Fix a mismatch on the issues this run created
(`gh issue edit`) and prove again.

## Report

Print the epic URL, one line per child (number, title, blockers, tier), and the
next step: `/epic-orchestrator:epic-orchestrator <epic> --dry-run` in Claude
Code or `$epic-orchestrator:epic-orchestrator <epic> --dry-run` in Codex.
