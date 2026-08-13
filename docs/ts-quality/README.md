# ts-quality — TypeScript Quality Gate

**Type:** Quality Gate | **Version:** 4.5.0 | **Depends on:** `toolu`

TypeScript `PostToolUse` quality checks registered into the toolu hook engine. Every TypeScript file the agent edits is checked on the spot.

## Install

```text
/plugin install ts-quality@toolu
```

## What It Provides

### Post-Edit Quality Checks

Every TypeScript file the agent edits is checked on the spot, contributing to toolu's quality gate. The checks are assembled from ordered `hooks/concerns/` fragments into one module at `SessionStart` and run only while this plugin is installed — **uninstall it and the TypeScript rules vanish, fail-closed.**

## Checks Enforced

### 1. Size Discipline

| Limit | Default | Configurable via |
|-------|---------|-----------------|
| File line limit | 300 lines | `lang.ts.maxFileLines` in toolu.config.json |
| Function line limit | 60 lines | `lang.ts.maxFnLines` |

Line counting excludes blank lines and comments.

### 2. No `../` Relative Imports

```typescript
// ❌ BANNED — relative parent imports
import { auth } from '../../lib/auth';

// ✅ CORRECT — use the @/ alias
import { auth } from '@/lib/auth';
```

Use the project's `@/` path alias for all cross-module imports.

This rule fires **only when the project actually defines a `@/*` path alias** (a
`compilerOptions.paths` entry in a `tsconfig.json` / `tsconfig.base.json` up-tree
from the edited file). A repo that uses plain NodeNext relative imports with no
alias configured has nothing to switch to, so `../` imports are left alone
there.

### 3. No `as` Type Assertions

```typescript
// ❌ BANNED — unsafe type assertions
const user = data as User;
const config = JSON.parse(raw) as AppConfig;

// ✅ CORRECT — use a type guard function
function isUser(data: unknown): data is User {
  return typeof data === 'object' && data !== null && 'id' in data;
}
if (!isUser(data)) throw new Error('Invalid user');
const user = data;

// ✅ CORRECT — use a Zod schema
const AppConfigSchema = z.object({
  port: z.number(),
  host: z.string(),
});
const config = AppConfigSchema.parse(JSON.parse(raw));
```

No `as` type assertions and no hand-rolled type guards — use Zod schemas or proper type guard functions.

### 4. Test Layout

```typescript
// ✅ CORRECT — colocated in flat __tests__/
// src/auth/__tests__/login.test.ts
import { login } from '../login';
// ...

// ❌ BANNED — inline tests in the same file
// src/auth/login.ts
describe('login', () => { ... });
```

Tests must live in a sibling `__tests__/` directory, kept flat (only `fixtures/`, `helpers/`, `mocks/`, `utils/` subdirs).

### 5. No Lint Suppression

```typescript
// ❌ BANNED
// @ts-ignore
// @ts-expect-error
// eslint-disable-next-line
// biome-ignore

// ✅ CORRECT — fix the actual problem
```

### 6. No Console Statements

```typescript
// ❌ BANNED in production code
console.log('debug info')`;
console.error('something broke');

// ✅ CORRECT — use a proper logger
logger.info('debug info');
logger.error('something broke');
```

### 7. No Thrown Literals

```typescript
// ❌ BANNED
throw 'Something went wrong';
throw 404;

// ✅ CORRECT — throw Error instances
throw new NotFoundError('User not found');
```

### 8. Duplicate Type Detection

Scans across the tree for duplicate type definitions. A type already defined elsewhere is a finding.

### 9. "Does Too Much" Heuristics

- **Too many factory functions** in a single file
- **Overloaded responsibilities** — a file exporting too many unrelated symbols

### 10. React-Specific Checks

- Hooks rules — no conditional hooks, hooks at top level
- Props object destructuring with proper types
- Toast/notification usage patterns
- Error boundary patterns

## How the Gate Works

1. **Agent edits a `.ts`/`.tsx` file** — `Write` or `Edit` tool call
2. **PostToolUse hook fires** — the assembled module checks the file
3. **Violation found** → gate goes **failing**, new task blocked until fixed
4. **Fix the violation** → gate clears, continue working

The gate is **multi-slot**: a failing test command and a failing file check are tracked independently — fixing one never silently masks the other.

## Configuration

Configure thresholds per project in `toolu.config.json`:

```json
{
  "version": 1,
  "lang": {
    "ts": {
      "maxFileLines": 400,
      "maxFnLines": 80
    }
  }
}
```

Precedence: project/user override → native linter's `max-lines` rule (`.oxlintrc.json`/`.eslintrc.json`) → built-in default (300/60). A value of `0` or `"off"` means "no override" and falls through.

## Usage Example

```text
# Session with the ts-quality plugin installed:

User: "Add an authentication middleware to src/middleware/auth.ts"
Agent: *writes the middleware*

> PostToolUse: Checking src/middleware/auth.ts...
> ❌ Gate: FAILING
>   - src/middleware/auth.ts:1: '../../lib/jwt' — use @/lib/jwt instead of ../
>   - src/middleware/auth.ts:28: as User — use a type guard or Zod schema
>   - src/middleware/auth.ts:55: console.log — use a logger
>   - src/middleware/auth.ts: file exceeds 300-line limit (342 lines)

# Agent is BLOCKED from starting new tasks until these are fixed:
#   - Change relative import to @/lib/jwt
#   - Replace as User with a Zod schema or type guard
#   - Replace console.log with logger.info
#   - Split the middleware into helper functions or a separate file

> PostToolUse: Checking src/middleware/auth.ts...
> ✅ Gate: PASSING
```

## Hooks

The fragments register into the core toolu dispatcher and run only while this plugin is installed. Uninstalling immediately removes the TypeScript rules:

```text
/plugin uninstall ts-quality@toolu
# → All TypeScript checks stop firing on the next edit
```
