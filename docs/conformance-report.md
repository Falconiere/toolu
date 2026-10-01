# Cross-host conformance report

**Issue:** [#212](https://github.com/Falconiere/toolu/issues/212)  
**Depth:** fixture-suite (hermetic temp projects; native dispatcher and Bun bootstrap)
**Runner:** `bun run test:conformance` → `tools/toolu-conformance/src/cli/run.ts`

## Pins (from [portable-core.md](./portable-core.md))

| Component | Pin |
|-----------|-----|
| Bun | `1.4.x` (workspace engines `>=1.4.0 <1.5.0`; CI/docs baseline `1.4.2`) |
| OpenCode CLI | `v2.0.12` (`$OPENCODE_BIN` or `command -v opencode`) |
| Plugin SDK | `@opencode/plugin@2.0.12` |

## Platforms

| OS | Status |
|----|--------|
| macOS | Supported (Bun 1.4.x and git for OpenCode) |
| Linux | Supported (same OpenCode prerequisites) |
| Windows | **N/A** — not probed for this port |

## Fixture suites

| Suite | What it proves |
|-------|----------------|
| `protected-files` | Native PreToolUse bundle blocks `.env` edit (`deny` or `ask`); **file bytes unchanged on `deny`** |
| `bootstrap-readiness` | A Bun registration bundle exits 0 without registry artifacts → `NotReady` |
| `permission-evaluate` | `permission.evaluate` handler + block mode → `deny` on protected `.env` |
| `surface-drift` | `bun run check:opencode-surface` clean |
| `spaces-cwd` | Project path with spaces still blocks protected edit |
| `live-opencode` | Optional live CLI probe (see below) |

## Live OpenCode lane (optional)

CI does **not** require a live OpenCode install. The `live-opencode` suite records **skip** and the matrix still exits 0.

To run locally:

```bash
export TOOLU_LIVE_OPENCODE=1
# optional: export OPENCODE_BIN=/path/to/opencode
bun run test:conformance
```

When `TOOLU_LIVE_OPENCODE=1`, the runner executes `opencode --version` (or `$OPENCODE_BIN --version`) and fails the matrix if that command is missing or non-zero.

For #276, the full fixture matrix passed with a temporary `@opencode/cli@2.0.12` binary, `TOOLU_LIVE_OPENCODE=1`, and a PATH containing only Bun and git. The live lane verifies the pinned CLI version; the separate OpenCode adapter test stages the npm catalog, bootstraps `ast-grep` into an isolated config root, and checks its in-process advisory with the same bash-free PATH.

## Isolation

Conformance fixtures use `TMPDIR` (recommended: `/Volumes/Projects/.tmp` locally) and bootstrap paths set `TOOLU_CONFIG_DIR` / isolated `HOME` so Claude/Codex host settings are not mutated.
