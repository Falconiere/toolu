# git-better

Token-lean git: a `gb` wrapper with lean defaults plus a cached repo-convention profile, surfaced via `PreToolUse` nudges registered into the toolu hook engine.

## Install

```
/plugin install git-better@toolu
```

Standalone, no dependencies.

## What it provides

- **`git-better` skill** — an always-active protocol: use `gb` for reads (`status` / `diff` / `log` / `show`) instead of raw git, and run `gb conventions` before writing a commit/PR/branch so it matches house style.
- **`gb` wrapper** (`scripts/git-better.sh`) — lean read defaults (`status -sb` no color, `diff --stat` first with lockfiles excluded, `log --oneline -n 20`, `show --stat HEAD`); a bare command applies the lean default while any path or git flag forwards verbatim. `gb conventions` emits a cached compact summary of the repo's commit/branch/PR conventions.
- **`git-lean-nudge` + `git-conventions-nudge` (`PreToolUse`)** — steer raw git reads toward `gb` and remind you to check conventions before a commit.
- **`git-byte-savings` (`PostToolUse`)** — instrumentation for tokens saved versus raw git output.

## The `gb` shim

`gb` is installed as a shim at `${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/toolu/bin/gb` at `SessionStart`; if it is not on `$PATH`, call it by that full path. It wraps the system `git` binary — no separate install. The hook modules register into the core toolu dispatcher and run only while this plugin is installed.
