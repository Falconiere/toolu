# agent-browser

Efficient, token-lean browser automation for agents via agent-browser (skill + CLI wrapper) — accessibility-tree snapshots with @eN refs instead of HTML/screenshots.

## Install

```
/plugin install agent-browser@toolu
```

Standalone, no dependencies.

## What it provides

- **`agent-browser` skill** (ALWAYS ACTIVE) — teaches the token-lean loop for driving a real, JS-rendered browser: `open → snapshot -i --json (@eN refs) → act → re-snapshot → get → close`, with a11y-tree-over-screenshots discipline, daemon reuse, and untrusted-page-text safety.
- **`agent-browser.sh` wrapper** — bakes token-lean defaults onto the read-heavy commands (`snapshot` → `-i --json --max-output 4000 --content-boundaries`; `get`/`find`/`diff` → `--json` + bounded output), passes everything else through, and offers a `--raw` bypass. Published at a stable path by a SessionStart hook.

Wraps the [agent-browser](https://github.com/vercel-labs/agent-browser) binary (vercel-labs, a native Rust CLI + daemon). Not bundled — install it once with `npm i -g agent-browser && agent-browser install` (also `brew install agent-browser` or `cargo install agent-browser`); `agent-browser install` fetches Chrome for Testing. The wrapper never runs a package manager. Under opencode the skill is auto-discovered by `tooling/install-opencode.sh`, which globs `plugins/*/skills` into `skills.paths`.
