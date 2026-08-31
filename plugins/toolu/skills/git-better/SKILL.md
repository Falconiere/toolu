---
name: git-better
description: "ALWAYS ACTIVE — Token-lean git. Use `gb` (status/diff/log/show) instead of raw git for reads, and run `gb conventions` BEFORE writing a commit/PR/branch so it matches house style. Cuts the tokens git output and convention re-discovery burn."
---

# git-better — Token-Lean Git Protocol

Raw git burns context two ways: bloated read output (`git diff` dumps every hunk incl. lockfiles + color codes) and re-discovering repo conventions (PR template, commit/branch style) on every commit. `gb` fixes both. **Always active.**

## Resolving `gb`

This skill ships its own wrapper — no separate plugin, no PATH shim, no SessionStart install step. When this skill loads, the harness reports its base directory (a line like "Base directory for this skill: /root/.claude/plugins/cache/toolu/toolu/<version>/skills/git-better"). Take that reported path, append `/scripts/git-better.sh`, and use it for every `gb <subcommand>` below — e.g. `bash "<reported base directory>/scripts/git-better.sh" status`. Resolve it once per session, not on every call. Working directly in a checkout of this repo instead (no skill load — e.g. running the bats suite): the script is at `plugins/toolu/skills/git-better/scripts/git-better.sh`.

## Pillar 1 — lean reads

| Instead of | Use | Why |
| :-- | :-- | :-- |
| `git status` | `gb status` | `status -sb`, no color |
| `git diff` | `gb diff` | `--stat` first, lockfiles excluded — then `gb diff <path>` to drill in |
| `git diff --cached` | `gb diff --cached` | any arg forwards verbatim |
| `git log` | `gb log` | `--oneline -n 20`; pass `-n`/`--format` to override |
| `git show` | `gb show` | `show --stat HEAD`; pass a ref/path to forward |

**Rule:** a *bare* `gb diff`/`gb show` applies the lean default; the moment you pass a path or any git flag it forwards verbatim (color off). `gb diff --full` / `gb show --full` force full hunks.

**Zero-setup raw-git fallbacks** (identical effect, no wrapper needed):
- `git -c color.ui=false status -sb`
- `git -c color.ui=false diff --stat -- . ':(exclude)*.lock' ':(exclude)*-lock.json'`
- `git -c color.ui=false log --oneline -n 20`
- `git -c color.ui=false show --stat`

## Pillar 2 — match house conventions

Before writing a commit message, branch name, or PR, run:

```
gb conventions          # compact summary
gb conventions --json    # full profile
```

It reports the repo's `commit_format` (e.g. conventional + scope + `(#N)`), `branch_naming` (e.g. `type/kebab`), PR template/sections, and release tooling — inferred from declared files *and* git history (most repos declare nothing; inference is the point). The profile is cached under `${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/toolu/git-better/conventions/` (Codex) or `${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/git-better/conventions/` (Claude Code) and refreshes only when convention files change, so repeated calls are free. Follow what it reports.

### Prose conventions (one-time distill)

If `gb conventions` lists files under `prose_pending` (e.g. `CONTRIBUTING.md`), read each **once**, distill the actionable rules, and persist them so they are never re-read:

```
printf '%s' "<your distilled rules>" | gb conventions --save-prose CONTRIBUTING.md
```

The text is read from STDIN and cached against the file's hash; it re-prompts only if the file changes.

## Recommended CLI levers (user-set)

These the skill cannot set for you — suggest them for scripted/repeated runs:

- `--allowedTools "Bash(git diff *)" "Bash(git log *)" "Bash(git status *)"` — git reads run without a permission round-trip.
- `--exclude-dynamic-system-prompt-sections` — moves machine-specific git/cwd lines out of the system prompt, improving prompt-cache reuse across runs.
