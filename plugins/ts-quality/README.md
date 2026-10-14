# ts-quality

TypeScript `PostToolUse` quality checks compiled into `toolu` and enabled by a registry manifest.

## Install

**Prerequisite:** the native [`toolu` binary](../../docs/install.md).

```
/plugin install ts-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every TypeScript file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are the `toolu-ts-quality` crate (`crates/ts-quality`), compiled into `toolu`. The `SessionStart` register hook publishes its `.json` manifest into toolu's registry, and removes the `.js` module and `.sh` bridge of earlier releases:

- File / function line limits (config-driven).
- No `../` relative imports — use the `@/` alias (only enforced when the project defines a `@/*` path alias in its tsconfig).
- No `as` type assertions and no hand-rolled type guards — use a type guard / Zod schema.
- Tests colocated in a flat `__tests__/`.
- Duplicate-type detection across the tree, plus "does too much" / too-many-factories heuristics.
- No `console` left in, no lint suppression, no thrown literals; React hooks/props, toast, and error-handling AST checks.

The rule runs inside the core toolu dispatcher only while this plugin is installed. Removing it drops its checks at the next tool call. `toolu ts-quality --help` lists the checks.
