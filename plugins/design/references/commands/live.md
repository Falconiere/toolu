# live — Iterate

**Invocation:** `/design live [target]`   **Status:** planned (deferred)

## Purpose
Iterate on a design directly in the browser — pick elements on the live page, generate alternative variants, and hot-swap them in place to compare options against the real running app. The goal is a tight visual feedback loop where you see changes instantly in context rather than rebuilding and reloading between every tweak.

## Status
Planned but deferred indefinitely. It requires a Node/Bun runtime plus browser automation to inspect and mutate a live page, which is out of scope for this zero-dependency plugin. Not yet implemented; if it ever ships, the flow and citations land then.

## For now
Until this ships, use **`/design critique`** or **`/design audit`** to evaluate the surface, then edit manually — the closest active path without a browser runtime — guided by the design law in `SKILL.md` and the cited knowledge base in `references/`.
