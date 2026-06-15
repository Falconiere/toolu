# ts-quality

TypeScript `PostToolUse` quality checks registered into the toolu hook engine.

## Install

```
/plugin install ts-quality@toolu
```

Requires the `toolu` plugin.

## What it provides

Every TypeScript file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks (assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart`):

- File / function line limits (config-driven).
- No `../` relative imports — use the `@/` alias.
- No `as` type assertions and no hand-rolled type guards — use a type guard / Zod schema.
- Tests colocated in a flat `__tests__/`.
- Duplicate-type detection across the tree, plus "does too much" / too-many-factories heuristics.
- No `console` left in, no lint suppression, no thrown literals; React hooks/props, toast, and error-handling AST checks.

The fragments register into the core toolu dispatcher and run only while this plugin is installed — uninstall it and the TypeScript rules vanish, fail-closed.
