# OpenCode statusline port (#359, OP-25) — Brainstorm

- **Outcome:** On OpenCode, the native `statusline-status` skill reports the selected toolu plugins, their startup state and readiness with actionable reasons, next to the existing repository, gate and Jev fields. The adapter also sends one structured `client.app.log` status entry. The persistent statusline is classified as Claude Code-specific, not claimed as an OpenCode feature.
- **Material defaults/non-goal:** The adapter persists a bounded per-project status record in its data root, and the status helper reads it through `TOOLU_CONFIG_DIR`. Out of scope: a TUI slot plugin, toasts, and any OpenCode version of `/statusline:setup`. Claude Code and Codex output stays unchanged.
- **Repository evidence:** Readiness lives only in plugin memory (`tools/toolu-opencode/src/plugin/enforcement.ts`, `hooks.ts`). `shell.env` sets `TOOLU_CONFIG_DIR` to the data root (`src/host/runtime-env.ts`). The startup ledger already uses atomic writes there (`src/bootstrap/ledger.ts`). The capability matrix marks `statusline.ui` as supported through `client.tui.showToast`, but nothing implements it. The pinned SDK `@opencode-ai/plugin/tui` exports slot plugins (`home_footer`, `sidebar_footer`) that load only in the interactive TUI. `AppLogData.body.extra` accepts structured metadata.
- **Risk:** A record left by an earlier session may be stale. The report prints its write time and project, and every startup overwrites it. Two instances in one project race on the record, and the last write wins. That is acceptable because both read the same selection.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Status channel | Status record in `<data root>/toolu/opencode-status.json` | Jev choice `snapshot` 0.91 vs env-only 0.09; plugin-process state is otherwise invisible to bash | Stale record; mitigated by write time and project in the report |
| UI | `statusline.ui` → `use: none`, plus a host-specific note | Jev choice `none-plus-note` 0.96; TUI slots cannot be verified by headless probes; a matrix `unsupported` status would need an unsupported probe verdict | None. This removes a support claim that nothing implemented |
| Logging | One `client.app.log` entry, `message: "toolu: status"`, with flat `extra` fields | `hostLog` in `src/plugin/context.ts`; SDK `extra?: Record<string, unknown>` | Secrets: fields come only from toolu's own verdict and plugin names, never from env values |
| Setup command | Keep it excluded from generation; replace the "no implementation in OP-10" reason with the final host-specific reason | `scripts/lib/constants.ts` `EXCLUDED_SURFACES` | None |

## Rejected

- **Env-only selection** (`TOOLU_PLUGIN_ROOT_*`): it shows which plugins were selected, but not their startup entries, artifacts or notes.
- **Helper recomputes selection**: it would duplicate adapter logic and could disagree with what actually started.
- **Startup toast**: it is supported, but appears only in the TUI and is not needed to meet the acceptance criteria.
- **New "no statusline API" probe**: it would need a live probe for a negative claim that the note already scopes.
