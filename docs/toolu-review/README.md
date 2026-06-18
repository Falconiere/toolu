# toolu-review — Pre-Push Code Review

**Type:** Workflow | **Version:** 0.1.0 | **Standalone** (no dependencies)

Project-tuned pre-push code review mirroring the CI review bot's checklist (correctness, security, perf, test coverage, doc accuracy). Records the `push-review` state so toolu's push gate passes.

## Install

```text
/plugin install toolu-review@toolu
```

## What It Provides

### \`toolu-review:review\` Skill

Reviews the branch diff against seven dimensions the CI review bot (the `claude[bot]` verdict comment) flags. Does **not** auto-fire on edits — run it explicitly before pushing, or when `pr-babysit` needs a reviewer.

## Usage Examples

### Basic Pre-Push Review

```text
toolu-review:review
```

Resolves the diff with `git diff --no-color <base>...HEAD`, reviews every hunk against the checklist, fixes findings in code, and records clean state for the push-review gate.

### Review Dimensions

| # | Dimension | What It Checks |
|:-:|-----------|---------------|
| 1 | **Correctness** | Logic, edge cases, error handling — no swallowed errors, no suppression comments (`@ts-ignore`, `eslint-disable`, `#[allow]`) |
| 2 | **Security** | Input validation, injection vectors, secrets exposure, unsafe file/symlink operations |
| 3 | **Performance** | Hot paths (per-render/per-hook work), needless subprocess spawns |
| 4 | **Test coverage** | Every new code path must have a colocated real-data test — missing tests are findings |
| 5 | **Doc/comment accuracy** | Comments must match behavior — no stale paths, no "one-time" on code that runs every invocation |
| 6 | **Tight test assertions** | Assert full identity, not loose suffixes (`*/statusline/statusline.sh`, not `*/statusline.sh`) |
| 7 | **In-session migration WARNs** | Breaking changes (moved paths, removed symlinks) must surface actionable hints, not silent failures |

### Workflow

```bash
# Resolve the diff (same way the push-review gate does)
git diff --no-color main...HEAD

# Review every changed hunk against the checklist
# Read surrounding code, grep for usage before claiming a finding

# Fix accepted findings in code, re-review until none remain

# Record the clean state (the SessionStart hook publishes this symlink;
# $CLAUDE_PLUGIN_ROOT is NOT exported to the Bash tool subshell).
bash "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/toolu-review/write-state.sh" \
  --findings-count 0 --reviewers '["toolu-review:review"]'
```

`write-state.sh` computes the gate's exact `diff_sha`/`base`/`slug`, bumps `review_round`, and writes `.claude/tmp/push-review/<branch-slug>.json` atomically. It's a harmless no-op when the toolu push-review gate is not installed.

### Unfixable Findings

If findings remain that need a human decision:

```bash
bash "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/toolu-review/write-state.sh" \
  --findings-count 3 --findings '[{"path":"src/auth.ts","line":42,"issue":"Needs product decision on session timeout"}]'
```

The gate keeps blocking — open findings mean the code is not ready to push.

### Integration with pr-babysit

When `pr-babysit` needs a reviewer, it picks from:

1. `caveman:cavecrew-reviewer` (when the caveman plugin is installed — preferred)
2. \`toolu-review:review\` (the CI-bot mirror — best for cutting bot rework)
3. Built-in `/code-review xhigh --fix` (always available)

## Output Format

Findings are severity-tagged:

```text
path:line: 🔴 blocker: <problem>. <fix>.
path:line: 🟡 should-fix: <problem>. <fix>.
path:line: 🔵 consider: <minor>. <fix>.
```

Ends with a verdict — either **Approved** (ready to push) or **Needs changes** (list the blockers).

## Why Run It Locally

The CI review bot (`claude[bot]`) edits its verdict comment **in place** — its header flips from "PR Review in Progress" to "Code Review —" and a `review / review` check can show `SUCCESS` *while low/nit findings remain unaddressed*. Running this review locally catches those before the first push, so the bot's verdict is clean on arrival.
