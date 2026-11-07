# toolu-review — Fallback review and push-review state

**Type:** Workflow | **Standalone** (no dependencies)

Agent skill that reviews a committed diff when native `code-review:review`
cannot complete, and records `push-review` state for toolu's push gate.
**Primary local pre-CI review is `code-review:review`** (`toolu code-review run`),
which shares the base-ref checklist in `.github/code-review-prompt.md` with CI.

## Install

```text
/plugin install toolu-review@toolu
```

## What It Provides

### `toolu-review:review` Skill

Fallback reviewer for the branch diff when native review is unavailable,
unconfigured, or incomplete. Prefer `code-review:review` first. This skill does
**not** auto-fire on edits — run it explicitly, or when `pr-babysit` /
`code-review:review` routes here.

## Usage

```text
toolu-review:review
```

Resolves `git diff --no-color <base>...HEAD`, reviews changed hunks for
introduced actionable defects, and — only after a clean committed review —
records push-review state:

```bash
toolu review write-state --findings-count 0 \
  --reviewers '["toolu-review:review"]'
```

Use `--repo` for another worktree. Host detection selects
`.claude/tmp/push-review/` or `.codex/tmp/push-review/`; set
`TOOLU_HOST_OVERRIDE` when markers are missing.

## Review bar

Same defect-first bar as native review: meaningful, actionable, introduced by
the change, demonstrated from evidence, author-would-fix. Omit style and
pre-existing issues. Prefer installed `ponytail-review` as a first pass when
available.

## Integration

| Situation | Reviewer |
| --- | --- |
| Normal pre-push / delivery | `code-review:review` |
| Native cannot complete | `toolu-review:review` (this skill) |
| CI bot on the PR | `falconiere/toolu-ghactions/code-review@v8` (separate runtime; shared prompt file) |

## Output

```text
path:line: severity: <problem>. <fix>.
```

End with `Approved` or `Needs changes`. Open findings keep the push gate blocked.
