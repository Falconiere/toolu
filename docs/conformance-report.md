# Cross-host conformance report

**Issue:** [#212](https://github.com/Falconiere/toolu/issues/212)  
**Depth:** fixture-suite (hermetic temp projects; real bash bridge and bootstrap)  
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
| macOS | Supported (Bash ≥5, `jq`, Bun 1.4.x) |
| Linux | Supported (same prerequisites) |
| Windows | **N/A** — not probed for this port |

## Fixture suites

| Suite | What it proves |
|-------|----------------|
| `protected-files` | Pre-tool bridge blocks `.env` edit (`deny` or `ask`); **file bytes unchanged on `deny`** |
| `bootstrap-readiness` | No-op bootstrap exit 0 without registry artifacts → `NotReady` |
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

## Isolation

Conformance fixtures use `TMPDIR` (recommended: `/Volumes/Projects/.tmp` locally) and bootstrap paths set `TOOLU_CONFIG_DIR` / isolated `HOME` so Claude/Codex host settings are not mutated.
