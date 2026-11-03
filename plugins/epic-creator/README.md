# epic-creator

Create an executable GitHub epic, or refresh an existing epic against current code and delivery evidence. Both skills produce cited decisions, observable acceptance criteria and focused test plans for unattended workers.

## Install

```
/plugin install epic-creator@toolu
```

**Prerequisite:** the native [`toolu` binary](../../docs/install.md) and an authenticated `gh`. Bun is not used or needed. See [docs/runtime.md](../../docs/runtime.md).

Requires the `brainstorm` plugin, which the installer adds. For Codex: `codex plugin add epic-creator@toolu`, or `toolu plugins install epic-creator --host codex`.

Keep `name`, `version` and `description` identical in `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json` and `.muse-plugin/plugin.json`.

## Crate

`crates/epic-creator` contributes the `toolu epic-creator` guide namespace. It is a Markdown-only plugin, so the namespace has no verbs.

## What it provides

- `/epic-creator:epic-creator <idea>` (Claude Code) or `$epic-creator:epic-creator <idea>` (Codex). Run it in the target repository's checkout. Options: `--repo owner/name`, which must name that checkout's repository, and `--dry-run`, which drafts and writes nothing.
- It runs `brainstorm` first, then drafts each child as one coherent PR, including affected callers, tests, fixtures and docs. Claims about existing code carry a `path:line` at a pinned base commit. Each acceptance criterion names an input, expected result and focused check. Bodies target 1200 characters; complete contracts take priority over cheaper routing.
- After one confirmation it creates the `epic` label when missing, the epic, its children, the native sub-issue links and the `blocked_by` dependencies. It makes one `gh api` call at a time and never repeats a create after a failure.
- It ends by reading the epic back with `toolu epic graph --dry-run` and `toolu epic route --dry-run`. Then it points you to `epic-orchestrator` to run the epic.

## Refresh an epic

Use `/epic-creator:epic-updater <epic>` (Claude Code) or
`$epic-creator:epic-updater <epic>` (Codex), for example:

```text
$epic-creator:epic-updater https://github.com/Falconiere/toolu/issues/832 --dry-run
```

The updater reads the parent, children, comments, dependencies and relevant
PRs, then checks their requirements against the current default branch. It
reports stale descriptions, missing behavior, conflicting contracts and gaps
in acceptance criteria or testing. A childless issue is audited as a proposal;
it is not reported as a completed or executable epic.

`--dry-run` saves local drafts and makes no GitHub writes. An update invocation
authorizes scoped body edits to the parent and its open children: it prepares
diffs, checks for concurrent edits, applies only changed bodies and reads them
back. Completed work is preserved. Active work is deferred for a coordinated
handoff; new issues, dependency changes and expanded scope need explicit
authorization. Interrupted writes are reconciled before any retry.

Creator and updater share one evidence workflow. Workers verify drift from
the cited base and reuse settled decisions in their delivery phases, keeping
required reviews and gates. Proposed tests remain distinct from observed
passing checks. The updater never launches workers or changes runtime policy.

GitHub only for now. Jira and Linear are not supported yet.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate with `muse plugins validate plugins/epic-creator`.
