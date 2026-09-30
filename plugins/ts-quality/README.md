# ts-quality

TypeScript `PostToolUse` quality checks registered into the toolu hook engine.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

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

The module runs inside the core toolu dispatcher, and only while this plugin is installed — uninstall it and the TypeScript rules vanish, fail-closed.
