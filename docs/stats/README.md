# stats — Usage Analytics Dashboard

**Type:** UI | **Version:** 0.3.0 | **Standalone** (only needs `jq`)

An on-demand `/stats` report of your **measured** Claude Code usage — tokens, estimated cost, and cache-hit rate — read straight from the session transcripts on disk. The first-party, no-estimate counterpart to the statusline's `wk:` sliver.

## Install

```text
/plugin install stats@toolu
```

Requires `jq` — without it, `/stats` prints a one-line notice and exits cleanly.

## What It Provides

### `/stats` Command

The default view is a **glyph dashboard**: a boxed header with a cache-hit gauge, a 14-day sparkline trend, and bar charts per project/model. Glyph-only (no ANSI) — renders identically wherever it is shown.

`--html` exports the same report as a self-contained, `prefers-color-scheme`-aware HTML file and opens it in the browser.

## Usage Examples

### Basic Reports

```text
/stats                       # Full digest: economics + all breakdowns
/stats --today               # Today's usage only
/stats --week                # Current ISO week
/stats --all                 # All-time (default window)
```

### Filtered Reports

```text
/stats --project toolu.sh    # One project (grouped on exact working dir path)
/stats --model opus          # One model tier
/stats --session <id>        # One session by ID
/stats --this-session        # Newest session (caveman-stats style)
/stats --since 2026-06-01    # Only sessions active on/after a date
/stats --limit 20            # Widen the top-N tables (default 10)
```

### Output Formats

```text
/stats --html                # Write self-contained HTML report and open in browser
/stats --json                # Machine-readable aggregate
/stats --rescan              # Ignore cache, recompute from transcripts
```

## What It Reports

### Economics

- **Tokens** — input + output + cache_creation (the rate-limit-pacing total)
- **Estimated cost** — per-model rates (sticker-price estimate, not an Anthropic bill)
- **Cache-hit %** — cache_read is tracked separately (dominates volume, billed ~0.1×)

### Time Windows

- **Today** — current local day
- **This week** — current ISO week (Mon–Sun, local time)
- **All-time** — every session on disk

### Per Project

Which repo burns the most — grouped on the exact working directory (`.cwd`), so repos sharing a basename stay separate.

### Per Model

Opus / Sonnet / Haiku split — the model-routing lever.

### Per Session

Top sessions by tokens/cost.

### toolu Activity

- Tool-mix (which tools were called most)
- Attributed-skill counts (workflow phase usage)
- Current quality-gate status
- Comemory count

## How It Works

Transcripts are the source of truth (`usage.sh`): assistant messages are deduped by `message.id` keeping the **final** streamed frame, priced per model (`pricing.sh`), and bucketed by local day.

Each session is rolled up once and cached (`scan.sh`), keyed on transcript mtime plus `schema_version` + `pricing_id` — so:

- A pricing change recomputes rather than serving stale cost
- An actively-written session busts its own cache (mtime advances)
- A deleted transcript drops from the totals (orphan caches are GC'd)

`aggregate.sh` reduces the rollups into report views (including a 14-day daily series for the trend). `render.sh` draws the glyph dashboard via `widgets.sh` bar/gauge/sparkline/box primitives. `render_html.sh` fills `templates/report.html` for `--html`.

## Cache Location

Session rollups at `${CLAUDE_CONFIG_DIR:-~/.claude}/stats/sessions/<id>.json`. Only reads `${CLAUDE_CONFIG_DIR:-~/.claude}/projects/**` and memoizes per-session rollups — no always-on hooks.

## Testing

`bats plugins/stats/__tests__` — real Claude Code transcripts projected to the fields stats reads, with all text stripped. No mocks.
