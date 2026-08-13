---
name: status
description: Use when the user asks for current repository, branch, working-tree, quality-gate, or comemory status in Codex.
---

# Status

Run `TOOLU_HOST_OVERRIDE=codex bash ../../scripts/status.sh` resolved from this
skill directory and return its output verbatim. The explicit override is
required because lifecycle-only plugin variables are not exported to ordinary
skill shell calls. The report includes only fields available from local
repository and toolu state. Do not invent Claude-only account, model, effort,
or context-window values when the active host does not expose them.
