# git-better — Token-Lean Git

**Type:** Workflow | **Version:** 0.1.0 | **Standalone** (no dependencies)

A `gb` wrapper with lean defaults plus a cached repo-convention profile, surfaced via `PreToolUse` nudges registered into the toolu hook engine.

## Install

```text
/plugin install git-better@toolu
```

## What It Provides

### 1. `git-better` Skill (Always Active)

Protocol requiring `gb` for reads (`status`/`diff`/`log`/`show`) instead of raw git, and `gb conventions` before writing a commit/PR/branch so it matches house style.

### 2. `gb` Wrapper (`scripts/git-better.sh`)

Lean read defaults with lockfile exclusion and color off:

| Instead of | Use | Why |
|:--|:--|:--|
| `git status` | `gb status` | `status -sb`, no color |
| `git diff` | `gb diff` | `--stat` first, lockfiles excluded |
| `git diff --cached` | `gb diff --cached` | any arg forwards verbatim |
| `git log` | `gb log` | `--oneline -n 20` |
| `git show` | `gb show` | `show --stat HEAD` |

A bare `gb diff`/`gb show` applies the lean default. Pass a path or any git flag and it forwards verbatim (color off). `gb diff --full`/`gb show --full` force full hunks.

### 3. Git Convention Nudge (`PreToolUse`)

Steers raw git reads toward `gb` and reminds you to check conventions before committing.

### 4. Byte-Savings (`PostToolUse`)

Instrumentation for tokens saved versus raw git output.

## Usage Examples

### Lean Reads

```bash
# Lean status (short, no color)
gb status

# Lean diff (stats first, lockfiles excluded)
gb diff

# Lean log (last 20 commits, oneline)
gb log

# Lean show (show --stat HEAD)
gb show

# Drill into a specific path
gb diff src/auth.ts

# Override the default log count
gb log -n 50

# Full hunks when you need them
gb diff --full

# Show a specific ref
gb show abc1234
```

### Zero-Setup Fallbacks (no shim needed)

```bash
# Identical effect — use these if gb shim isn't on PATH:
git -c color.ui=false status -sb
git -c color.ui=false diff --stat -- . ':(exclude)*.lock' ':(exclude)*-lock.json'
git -c color.ui=false log --oneline -n 20
git -c color.ui=false show --stat
```

### Check Conventions Before Committing

```bash
# Compact summary of repo conventions
gb conventions

# Full machine-readable profile
gb conventions --json
```

Output example:

```text
Commit format: conventional (feat:, fix:, refactor:, ...) + scope + (#issue)
Branch naming: type/kebab-case
PR template: .github/pull_request_template.md (checklist present)
```

### Prose Convention Distillation

If `gb conventions` lists files under `prose_pending` (e.g. `CONTRIBUTING.md`):

```bash
# Read the file ONCE, distill actionable rules, persist the summary:
printf '%s' 'Always run cargo fmt and cargo clippy before committing.
Branch names: feat/name, fix/name, chore/name.
Commit messages: conventional commits only.' | gb conventions --save-prose CONTRIBUTING.md
```

The text is cached against the file's hash — it re-prompts only if the file changes.

### Before Writing a Commit Message

```bash
# Always check conventions first — the skill auto-nudges if you don't
gb conventions
# → commit_format: conventional
# → scope: feat:, fix:, refactor:, chore:, docs:, test:
# → branch_naming: type/kebab-case

# Then write your commit in the right format:
git commit -m "feat(auth): add JWT middleware with refresh token support"
```

### Recommended CLI Levers

These the plugin cannot set for you — suggest them for scripted/repeated runs:

```bash
# Allow git reads without a permission round-trip
--allowedTools "Bash(git diff *)" "Bash(git log *)" "Bash(git status *)"

# Exclude machine-specific git/cwd lines from the system prompt
--exclude-dynamic-system-prompt-sections
```

## Hooks

| Hook | Event | Purpose |
|------|-------|--------|
| `git-lean-nudge` | PreToolUse | Steers raw `git` toward `gb` for reads |
| `git-conventions-nudge` | PreToolUse | Reminds to check conventions before committing |
| `git-byte-savings` | PostToolUse | Tracks tokens saved vs. raw git output |

Hooks register into the core toolu dispatcher at `SessionStart` — they run only while this plugin is installed.

## gb Shim Location

`gb` is installed as a shim at:

```
${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/bin/gb
```

Created at `SessionStart`. If it's not on `$PATH`, call it by the full path or use the zero-setup fallbacks above.

## Why Token-Lean Git?

Raw git burns context two ways:

1. **Bloated read output** — `git diff` dumps every hunk including lockfiles and color codes. `gb diff` starts with `--stat`, excludes lockfiles, and shows full hunks only on demand.
2. **Convention re-discovery** — every commit requires re-reading PR templates, commit format rules, and branch naming conventions. `gb conventions` caches the profile and refreshes only when convention files change.

Result: fewer tokens spent on git I/O = more context available for actual code.
