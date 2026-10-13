# ts-quality

TypeScript `PostToolUse` quality checks compiled into `toolu` and enabled by a registry manifest.

## Install

**Prerequisite:** the native [`toolu` binary](../../docs/install.md).

```
/plugin install ts-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every TypeScript file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are the `toolu-ts-quality` crate (`crates/ts-quality`), compiled into `toolu`. The `SessionStart` register hook publishes its manifest into toolu's registry, plus a small `.sh` bridge that lets the Bun post-tools dispatcher run the same rule until #425. It also removes the Bun module of earlier releases:

- File / function line limits (config-driven).
- No `../` relative imports — use the `@/` alias (only enforced when the project defines a `@/*` path alias in its tsconfig).
- No `as` type assertions and no hand-rolled type guards — use a type guard / Zod schema.
- Tests colocated in a flat `__tests__/`.
- Duplicate-type detection across the tree, plus "does too much" / too-many-factories heuristics.
- No `console` left in, no lint suppression, no thrown literals; React hooks/props, toast, and error-handling AST checks.

The rule runs inside the core toolu dispatcher only while this plugin is installed. Removing it drops its checks at the next tool call. `toolu ts-quality --help` lists the checks.

## OpenCode

OpenCode support is withdrawn. The notes below are not a supported install path.

Add `ts-quality` to the project selection and restart OpenCode:

```json
{ "version": 1, "enabled": ["toolu", "ts-quality"] }
```

The project must be a git repository with a tracked `tsconfig*.json`, a Bun, pnpm, yarn, or npm lock file, and that package manager available. Completed `write`, `edit`, and `apply_patch` calls check changed `.ts` and `.tsx` files. The installed `ast-grep` CLI runs structural rules; OpenCode selects its TSX parser for `.tsx` files. Without `ast-grep`, those structural rules are skipped. A multi-file patch checks every changed TypeScript destination even if an earlier file fails, and clears prior entries for deleted files and moved sources. Other extensions and disabled plugins do not run these checks. The existing linked-worktree rule skips TypeScript quality checks there.

Violations are appended to the completed tool result and recorded in the project's `.opencode/tmp/quality-gate-status.json`; they do not undo the edit. A failing entry blocks later commit and push attempts until a clean edit or deletion clears it. The pinned-host proof is `bun run smoke:opencode-ts-quality` in the toolu checkout.
