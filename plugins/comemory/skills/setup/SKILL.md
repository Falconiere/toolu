---
name: setup
description: Use when the comemory binary or repository memory/index integration needs first-time setup, repair, or an explicitly confirmed upgrade.
---

# Set up comemory

Read [the canonical setup workflow](../../workflows/setup.md) completely, then
execute it with `TOOLU_HOST_OVERRIDE=codex` in the setup script's environment.
Resolve its script path relative to the workflow file. The explicit override is
required because Codex lifecycle variables are not exported to ordinary skill
shell calls. Use `$comemory:setup` when telling the user how to rerun this
skill.
