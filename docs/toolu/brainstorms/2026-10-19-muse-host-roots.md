# Muse host detection and roots — Brainstorm

**Date:** 2026-10-19  **Mode:** Delivery  **Issue:** Falconiere/toolu#542

## Outcome

`toolu --host muse` names an explicit fifth host. Muse hooks detect from Muse's
own environment before the shared `PLUGIN_ROOT` alias can select Codex. The
existing `Roots` API yields Muse config, project, plugin and command values.

## Evidence and decisions

- The epic's Muse 1.4.10 probe observed `MUSE_PLUGIN_ID`, `MUSE_SESSION_ID`,
  `MUSE_PLUGIN_ROOT`, `MUSE_PLUGIN_DATA_DIR` and both Claude and Codex aliases.
  The issue fixes Muse precedence above `PLUGIN_ROOT`; keep the current
  override and unique-event precedence.
- The probe found project skills under `.agents/skills/`, settings at
  `~/.config/muse/settings.json`, and local plugin installation through
  `muse plugins install <path>`. The installed Muse 1.4.10 CLI confirms
  `plugins install <path>` and `plugins remove <id>`.
- Reuse the shared PascalCase event mapping; Muse's four extra native events
  stay unmapped. Keep `Host::ALL` order with Muse after Hermes.
- Use `/namespace:name` for Muse invocation, matching the three existing
  slash-style hosts. The issue asks to choose between slash and dollar; Jev
  favored slash (0.95 relative probability), and Meta's Muse Code extending
  guide documents slash shortcuts for skills. Plugin namespace spelling still
  needs a live check.
- Resolve `MUSE_PLUGIN_ROOT` and `MUSE_PLUGIN_DATA_DIR` before the existing
  Claude compatibility aliases. Jev favored this minimal fallback over adding
  a Codex alias fallback (0.75 relative probability). Muse's own variables
  are the expected live path.

## Scope and risk

This issue changes only protocol host/event names, runtime detection/roots,
their tests and host fixture, and the CLI host snapshot. Payload and encoder
semantics belong to #543/#544. An accidental exhaustive match or snapshot
  change elsewhere may require a small compatibility edit. The key behavior
  risk is the exact namespace spelling of a plugin slash shortcut.

## Handoff

Write the design spec with real-input evidence for each acceptance criterion.
