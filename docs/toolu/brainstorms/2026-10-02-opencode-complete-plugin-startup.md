# OpenCode complete plugin startup — Brainstorm

**Date:** 2026-10-02   **Issue:** #342 (OP-08 of epic #334)   **Mode:** Delivery, Full path

## Capsule

- **Outcome:** On OpenCode plugin init, every selected plugin runs all of its supported SessionStart entries in dependency order. Each entry's registry, helper and context contributions come back structured. Readiness passes only when this run's contributions for every selected plugin succeeded and are current on disk. Contributions of plugins that are no longer selected are removed when toolu still owns them.
- **Material defaults / non-goals:** Context is collected but not delivered to the model (OP-07). Helper environment, HOME isolation and shared data roots stay with OP-09. Surface install is OP-11. Any startup failure still denies every tool call (the deny-all from #336), now with a per-plugin diagnosis.
- **Repository evidence:** `bootstrap/entrypoint.ts` picks `register.js` over `session-start.js`, so the quality plugins' `check-toolu` entry never runs. `readiness.ts` passes when any registry file or `.session-start-ready` exists. `runRegisterHook` exits 0 after a failed write, and `publishWrapper` publishes nothing, silently, when its source is missing. Every `hooks.json` SessionStart command is the generated `launcherCommand`, gated by `check:hooks-json`.
- **Risk:** An entry that silently stops reporting would still pass. A catalog-wide real-subprocess test pins each plugin's expected contributions. Core bundles must be rebuilt (`build:plugins`), and Claude/Codex behavior must not change: the report channel is inert unless the OpenCode bootstrap sets it.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Which entries run | Every SessionStart hook in `hooks.json` whose matcher covers `startup`, accepted only in the exact generated launcher form, run in file order | One routing source shared with Claude/Codex; `launcherCommand` is exported from `@toolu/core/launcher` | A hand-written command is reported as unsupported (NotReady), as the legacy `.sh` check did |
| How contributions are learned | Core `runRegisterHook` and `publishWrapper` append strict JSON records to the file named by `TOOLU_STARTUP_REPORT`. Only the OpenCode bootstrap sets it, with a fresh file per entry run | Partial failures exit 0 today, so only a structured record exposes them | An entry that stops reporting slips through; the catalog test covers it |
| Readiness | Per plugin, from this run only: exit 0, parseable output, every record `ok`, each artifact verified on disk (registry bytes equal the bundle, helper symlink points at the bundle, both inside the selected plugin) | The acceptance criteria name stale markers, unrelated modules, partial failure and missing artifacts | Stricter than before. A user-owned helper file is kept and reported as a diagnostic, not a failure |
| Disabled cleanup | An ownership ledger records the published helpers. Registry files under `<name>@toolu__` of catalog or ledger plugins that are no longer selected are removed. Helpers are removed only while they are still the recorded symlink | Mirrors `runRegisterHook`'s own prefix prune and `publishWrapper`'s user-file rule | A data root shared by projects with different selections is last-startup-wins (OP-09) |
| Order | Topological order over the dependency closure: names sorted, dependencies first, cycles rejected with the path | The current BFS order runs dependents before toolu | — |
| Deadlines | Per-entry deadline (existing 120 s), plus a cancellation `AbortSignal`. Enforcement passes a 180 s total budget | Plugin init blocks the host | — |
| `.session-start-ready` | Removed: nothing reads it once readiness comes from records | Only the OpenCode readiness check read it | None on Claude/Codex |

## Alternatives rejected

- **Hand-written per-plugin startup table** in the OpenCode package: it duplicates the `register.ts` declarations, so it needs a second drift gate.
- **New `plugin.json` field:** Claude validates `plugin.json`, and the field would duplicate `hooks.json`.
- **Filesystem diff around each entry:** it cannot tell a failed write from an intentional no-op, and it races concurrent sessions.
- **Static expected-artifact list:** this is the table above under another name.
- **Symlink scan for disabled helpers:** it misses dangling links whose plugin directory moved.
- **Dispatch-time selection gate:** it changes core gating for every host, and modules of disabled plugins still sit on disk.

## Jev

Evidence: `state.md` (scope, acceptance criteria, code facts above). Answers: entries → `hooks_json` (1.0); records → `report_channel` (0.70, none_fit 0.18, which drove the catalog test); disabled → `ledger_prune` (0.74); marker → `remove` (0.66).
