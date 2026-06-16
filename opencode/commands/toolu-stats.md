---
description: Report measured token / cost / cache usage and toolu activity from session transcripts.
agent: build
---

# Usage stats

Report measured token / cost / cache usage and toolu activity from the session transcripts.

Run the report and show its output verbatim:

```sh
bash "${OPENCODE_CONFIG_DIR:-$HOME/.config/opencode}/stats/stats.sh" $ARGUMENTS
```

Pass any arguments the user gave through unchanged. Common flags:
`--today | --week | --all`, `--project <label|path>`, `--model <substr>`, `--session <id>`, `--this-session`, `--json`, `--html`, `--rescan`, `--since <YYYY-MM-DD>`, `--limit N`.

Do not summarize or reformat the output — print exactly what the script emits (the default view is a glyph dashboard). With `--html` the script writes a self-contained HTML report, opens it in the browser, and prints the file path — relay that path verbatim. Cost figures are sticker-price estimates, not a bill.
