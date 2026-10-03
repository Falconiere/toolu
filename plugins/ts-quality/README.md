# ts-quality

TypeScript `PostToolUse` quality checks registered into the toolu hook engine.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x, resolved from `TOOLU_BUN`, `PATH`, or `~/.bun/bin/bun`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install ts-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every TypeScript file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks run as one bundled TypeScript module (`hooks/dist/post-tool-use.js`, built from `hooks/src/`), which the `SessionStart` register hook publishes into toolu's registry:

- File / function line limits (config-driven).
- No `../` relative imports — use the `@/` alias (only enforced when the project defines a `@/*` path alias in its tsconfig).
- No `as` type assertions and no hand-rolled type guards — use a type guard / Zod schema.
- Tests colocated in a flat `__tests__/`.
- Duplicate-type detection across the tree, plus "does too much" / too-many-factories heuristics.
- No `console` left in, no lint suppression, no thrown literals; React hooks/props, toast, and error-handling AST checks.

The module runs inside the core toolu dispatcher only while this plugin is enabled. Removing it from the selection drops its checks at the next session start.

## OpenCode

Add `ts-quality` to the project selection and restart OpenCode:

```json
{ "version": 1, "enabled": ["toolu", "ts-quality"] }
```

The project must be a git repository with a tracked `tsconfig*.json`, a Bun, pnpm, yarn, or npm lock file, and that package manager available. Completed `write`, `edit`, and `apply_patch` calls check changed `.ts` and `.tsx` files. The installed `ast-grep` CLI runs structural rules; OpenCode selects its TSX parser for `.tsx` files. Without `ast-grep`, those structural rules are skipped. A multi-file patch checks every changed TypeScript destination even if an earlier file fails, and clears prior entries for deleted files and moved sources. Other extensions and disabled plugins do not run these checks. The existing linked-worktree rule skips TypeScript quality checks there.

Violations are appended to the completed tool result and recorded in the project's `.opencode/tmp/quality-gate-status.json`; they do not undo the edit. A failing entry blocks later commit and push attempts until a clean edit or deletion clears it. The pinned-host proof is `bun run smoke:opencode-ts-quality` in the toolu checkout.
