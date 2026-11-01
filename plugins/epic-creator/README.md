# epic-creator

Turn an idea into a GitHub epic the epic-orchestrator engine can run: brainstorm it, split it into one-PR sub-issues with cited decisions and runnable checks, confirm once, then create the epic, native sub-issues and dependencies.

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
- It runs `brainstorm` first, then drafts every sub-issue under fixed body rules. Each claim about the code carries a `path:line` at a pinned base commit. Each acceptance criterion names its check. Bodies stay near 1500 characters, so workers read less and the engine routes them as Standard.
- After one confirmation it creates the `epic` label when missing, the epic, its children, the native sub-issue links and the `blocked_by` dependencies. It makes one `gh api` call at a time and never repeats a create after a failure.
- It ends by reading the epic back with `toolu epic graph --dry-run` and `toolu epic route --dry-run`. Then it points you to `epic-orchestrator` to run the epic.

GitHub only for now. Jira and Linear are not supported yet.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`. Validate with `muse plugins validate plugins/epic-creator`.
