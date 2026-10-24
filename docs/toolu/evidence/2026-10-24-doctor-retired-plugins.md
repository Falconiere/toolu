# Evidence: retired marketplace plugins in doctor and `plugins remove` (#615)

**Date:** 2026-10-24   **Plan:** docs/toolu/plans/2026-10-24-doctor-retired-plugins.md   **Spec:** docs/toolu/specs/2026-10-24-doctor-retired-plugins-design.md

Live run of the branch's `target/debug/toolu` against the real Claude Code 2.1.310 and Codex 0.164.0 CLIs, each in an isolated config home under `/private/tmp` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`). The Claude home installs the current `toolu@toolu` from this worktree's marketplace, which no longer lists context7, and records the real 7.11.0 context7 copied from this machine's cache, as Claude Code recorded it. The Codex home has a local `toolu` marketplace without context7 and a leftover `[plugins."context7@toolu"]` table.

- **AC-6 (Claude Code):** `toolu plugins remove context7 --host claude --yes` exits 0 and `claude plugin list --json` and `enabledPlugins` no longer list `context7@toolu`.
- **AC-6 (Codex):** `toolu plugins remove context7 --host codex --yes` exits 0 and `config.toml` no longer has the `context7@toolu` table; the plugin cache is empty.
- **AC-8:** doctor warns `retired plugin context7 is no longer in the toolu marketplace` with `fix: toolu plugins remove context7 --host claude --yes`; after that exact command, doctor reports `ok skew: plugin versions match` and exits 0.

The `binary` and `reachability` rows describe the Homebrew `toolu` that `PATH` resolves, not the debug binary that ran; they are unrelated to this change.

## Output

```text
## Versions

$ claude --version
2.1.310 (Claude Code)
[exit 0]

$ codex --version
codex-cli 0.164.0
[exit 0]

$ /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail/target/debug/toolu --version
toolu 8.2.0
[exit 0]

## Claude Code

$ claude plugin marketplace add /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail
Adding marketplace…✔ Successfully added marketplace: toolu (declared in user settings)
[exit 0]

$ claude plugin install toolu@toolu
Installing plugin "toolu@toolu"...✔ Successfully installed plugin: toolu@toolu (scope: user)
[exit 0]

$ cat /private/tmp/toolu615-claude/cfg/plugins/cache/toolu/context7/7.11.0/.claude-plugin/plugin.json
{
  "name": "context7",
  "version": "7.11.0",
  "description": "Library documentation & code-example lookup via Context7 (skill + REST wrapper). Standalone, no dependencies.",
  "author": {
    "name": "Falconiere Barbosa",
    "email": "hello@falconiere.io"
  },
  "homepage": "https://github.com/falconiere/toolu",
  "repository": "https://github.com/falconiere/toolu.git",
  "license": "MIT",
  "keywords": [
    "claude-code",
    "context7",
    "documentation",
    "skills"
  ]
}
[exit 0]

$ claude plugin list --json | jq -c ...
{"id":"context7@toolu","version":"7.11.0","enabled":true,"scope":"user"}
{"id":"toolu@toolu","version":"8.2.0","enabled":true,"scope":"user"}

$ /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail/target/debug/toolu --host claude doctor
ok binary: toolu 8.2.0 protocol 2 at /opt/homebrew/Cellar/toolu/8.2.0/bin/toolu; upgrade with: brew upgrade toolu
ok reachability: native toolu at /opt/homebrew/Cellar/toolu/8.2.0/bin/toolu
ok host: claude config /private/tmp/toolu615-claude/cfg project /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail
ok config: config is valid
ok plugins: 2 toolu plugins
warn skew: retired plugin context7 is no longer in the toolu marketplace
  fix: toolu plugins remove context7 --host claude --yes
ok registry: no findings
ok tools: required tools are present
[exit 0]

$ /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail/target/debug/toolu plugins remove context7 --host claude --yes
  - context7         removed
[exit 0]

$ claude plugin list --json | jq -c ...
{"id":"toolu@toolu","version":"8.2.0","enabled":true,"scope":"user"}

$ jq .enabledPlugins /private/tmp/toolu615-claude/cfg/settings.json
{
  "toolu@toolu": true
}
[exit 0]

$ /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail/target/debug/toolu --host claude doctor
ok binary: toolu 8.2.0 protocol 2 at /opt/homebrew/Cellar/toolu/8.2.0/bin/toolu; upgrade with: brew upgrade toolu
ok reachability: native toolu at /opt/homebrew/Cellar/toolu/8.2.0/bin/toolu
ok host: claude config /private/tmp/toolu615-claude/cfg project /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail
ok config: config is valid
ok plugins: 1 toolu plugins
ok skew: plugin versions match
ok registry: no findings
ok tools: required tools are present
[exit 0]

## Codex

$ cat /private/tmp/toolu615-codex/home/config.toml
[marketplaces.toolu]
source_type = "local"
source = "/private/tmp/toolu615-codex/mkt"

[plugins."context7@toolu"]
enabled = true
[exit 0]

$ /Volumes/Projects/.herdr/worktrees/toolu/feat-615-doctor-retired-marketplace-plugins-fail/target/debug/toolu plugins remove context7 --host codex --yes
  - context7         removed
[exit 0]

$ cat /private/tmp/toolu615-codex/home/config.toml
[marketplaces.toolu]
source_type = "local"
source = "/private/tmp/toolu615-codex/mkt"
[exit 0]

$ ls -R /private/tmp/toolu615-codex/home/plugins/cache/toolu
[exit 0]
```
