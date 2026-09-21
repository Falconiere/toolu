# agent-guardrails

The kit's structural gate. One module, copied verbatim into every generated project,
enforcing the rules a linter structurally cannot see: the folder tree, per-domain shape,
colocated tests, barrels, banned dependencies, required files, committed secrets, and
contextual code patterns. It also protects lint integrity by rejecting source directives
that disable dead-code enforcement.

Anything oxlint / oxfmt / clippy / rustfmt already enforces stays with them. Two enforcers
of one rule is how ceilings drift apart.

## Two paths, one module

This is the single most common source of confusion, so it is worth stating plainly:

| | Path | What lives there |
| --- | --- | --- |
| **Source** | `guardrails/` — here, in the kit | The one copy anyone edits. Includes `__tests__/`. |
| **Destination** | `scripts/guardrails/` — in a generated project | What a scaffold copies in. No `__tests__/`. |

`CORE.md`, every stack's `SETUP.md`, the `.claude/settings.json` hooks, and each stack's
`lefthook.yml` all name **`scripts/guardrails/`**, because they describe a generated
project. A root-level `guardrails/` path and a `scripts/guardrails/` path are both correct;
they are source and destination, not old and new.

Until 2026-08 each stack also carried its own byte-identical copy at
`stacks/<stack>/templates/scripts/guardrails/` — five copies, 135 files, kept honest only
by a `diff -r` in CI. Those are gone. There is one source now, and the scaffold reads it
directly.

## What a scaffold copies

The **manifest** — and never `__tests__/`:

```bash
mkdir -p scripts/guardrails
for item in run.sh lib checks patterns schema.json oxlint-plugin; do
  cp -R "$KIT/guardrails/$item" scripts/guardrails/
done
chmod +x scripts/guardrails/run.sh
```

Copy the **whole** set: `run.sh` sources `lib/` and `checks/` from beside itself, so
copying it alone fails at runtime. `__tests__/` is excluded on purpose — it is a
deliberately-violating fixture tree, and shipping it would trip the very gate it exists to
test.

The project then gets its stack's `guardrails.config.json` (from
`stacks/<stack>/templates/`), which is the only per-stack part of the whole arrangement.

## Layout

```
guardrails/
├── run.sh              # entry point — repo · --file · --hook · --stop modes
├── lib/                # config.sh (load + validate), report.sh (output, exit codes),
│                       #   workspace.sh (monorepo dispatch)
├── checks/             # 14 checks, one file each
├── oxlint-plugin/      # house rules that run inside oxlint, as the file is written
├── patterns/rust/      # ast-grep rules for what clippy doesn't cover
├── schema.json         # the guardrails.config.json contract
├── workspace.schema.json  # the guardrails.workspace.json contract (monorepos)
└── __tests__/          # fixtures, plugin and latency suites — kit only, never shipped
```

## Monorepos

A single-repo project has one `guardrails.config.json` at its root. A workspace
has one **per package**, plus a `guardrails.workspace.json` at the root naming
them:

```jsonc
{
  "version": 1,
  "packages": ["packages/api", "packages/database"],
  "bannedDeps": ["axios"],
  "secrets": { "neverTracked": [".dev.vars"] },
  "shadowConfigs": []
}
```

`run.sh` re-execs itself once per package, with the working directory inside it
and `GR_PATH_PREFIX` set, so every check runs in the single-root shape it was
written for and violations still read `packages/database/src/foo.ts`. The four
repo-level checks — `banned-deps`, `shadow-configs`, `required-files`,
`secrets` — run once at the root against the manifest.

Two rules that are load-bearing rather than stylistic:

- **There must be no `guardrails.config.json` at a workspace root.** The oxlint
  plugin resolves that filename from `process.cwd()`, so a root file under that
  name would let a root-level oxlint run lint every package against one config
  and silently disable whatever each package declares `ownedByLinter`. Under
  the manifest's own name, a root-level oxlint run fails closed at plugin load
  instead. Run oxlint per package (`bun run --filter '*' lint`). Having both
  files at the root exits 3.
- **Each package guards its own secrets.** wrangler reads `.dev.vars` beside
  each `wrangler.jsonc`, so every package config lists its own; the manifest
  lists only root-level ones. Listing a package's secret in both makes it
  report twice.

It fails closed in six ways, all exit 3, because each would otherwise leave a
source tree unguarded while the gate reported green: a listed package with no
config; a listed package directory that is absent; an empty `packages` array; a
package path containing `..` (run.sh cd's into these); a package that exists but
is **not** listed, in repo mode or via a `--file` path inside it — its stray
`guardrails.config.json` is the evidence; and both documents at the root.

A `--file` path at the workspace **root** is not one of them. lefthook expands
`{staged_files}` across whatever the commit touched, so treating `package.json`
as an unguarded tree would block every commit that edits it, and there is no
file-addressable check at a workspace root to run against it regardless.

## One rule, one enforcer

Rules a linter can own run inside **oxlint** via the house plugin, so they fire as the
file is written rather than at commit time. Everything else runs here; the one deliberate
cross-boundary integrity check is described below.

`ownedByLinter` in `guardrails.config.json` declares the split, and `gr_selected()` in
`run.sh` skips whatever the linter owns. A check id is the unit of ownership, which is why
`folder-readmes` is split out of `folder-tree` and `test-tree` out of `colocated-tests`: an
id may not straddle what a linter can see and what it cannot.

| | TS stacks (`console` · `marketing` · `backend-ts` · `database-ts` · `expo`) | `rust` |
| --- | --- | --- |
| `ownedByLinter` | `folder-tree` · `colocated-tests` · `no-barrels` · `filename-case` · `patterns` | *(empty)* |
| Runs in oxlint | those five — three as house rules (`house/folder-tree`, `house/colocated-tests`, `house/no-barrels`), `filename-case` as the built-in `unicorn/filename-case`, and `patterns` as `house/no-bare-fetch` + `house/no-hardcoded-hex` + `house/no-module-scope-database` | nothing — oxlint cannot parse Rust |
| Runs here | the remaining nine | all fourteen |
| Needs ast-grep | no — pattern rules run in oxlint | **yes** (`cargo install ast-grep --locked`) |

Two of the five are not one-rule-per-id, which is why `validate-templates.sh` special-cases
them when it checks that a linter-owned id is actually configured: `filename-case` maps to an
oxlint built-in rather than anything in `oxlint-plugin/`, and `patterns` maps to three house
rules at once. The plugin exports six rules — `folder-tree`, `colocated-tests`,
`no-bare-fetch`, `no-barrels`, `no-hardcoded-hex`, `no-module-scope-database` — which is a
different set from the five in the `ownedByLinter` list above; the counts are not meant to
match, and reading one as the other is the mistake this paragraph exists to prevent.

`lint-suppressions` is deliberately independent of that ownership split. TypeScript/oxlint
and Cargo/clippy decide whether a declaration is unused; this check only prevents source
from silencing those decisions with a blanket disable, a `no-unused-vars` directive, or
Rust attributes that lower or consume `dead_code` directly or through the
`unused` lint group. Direct, `cfg_attr`, single-line, inline-block, conditional,
and multiline forms are covered, including nested macro brackets or comments
inside a Rust attribute and Rust's equivalent raw-identifier spellings. The
scanners distinguish executable syntax from TypeScript strings or rendered JSX
text and from Rust comments, strings, or character literals, so documentation
examples remain legal. JSX/template expressions remain executable context even
when regex literals contain braces, and TSX generic arrows are not mistaken for
markup, including multiline parameter lists with regexes or comments around
their boundaries. A comment-free lexical context keeps ordinary,
postfix-expression, and completed-regex division as code even when block-comment
trivia intervenes; a control-parenthesis stack keeps consequent regex statements
in regex context. A
token-boundary check extends that handling to `do` and `else` without confusing
public or private keyword-named property methods such as `object.if()` or
`this.#if()`. A scoped disable for an unrelated rule is still available.

## What differs per stack

Only `guardrails.config.json`. The module itself is identical everywhere:

| | `console` | `marketing` | `backend-ts` | `database-ts` | `expo` | `rust` |
| --- | --- | --- | --- | --- | --- | --- |
| `srcRoot` | `src` | `src` | `src` | `src` | `src` | `src` |
| `src.topLevel` | app · ui · domains · api · utilities · providers · constants · types | pages · layouts · sections · ui · content · utilities · constants · types | rpc · routes · domains · utilities · constants · types | client · schema · constants · types | ui · domains · api · utilities · providers · constants · types | domains · config · error · utilities · cli · http |
| Tests | `__tests__/` colocated | `__tests__/` colocated | `__tests__/` colocated | `__tests__/` colocated | `__tests__/` colocated | sibling `tests/` |
| File / function ceiling | 300 / 50 | 300 / 50 | 300 / 50 | 300 / 50 | 300 / 50 | 500 / 100 |
| Banned deps | axios · yup · joi · valibot · superstruct · ajv | same | same | same | same | *(none listed)* |
| Also carries | — | `requiredFiles` | `requiredFiles` | workspace package | — | `filenameCase` |

The ceilings are cross-checked against what oxlint and `clippy.toml` actually enforce, by
`scripts/validate-templates.sh` — two numbers for one rule is how they part company.

## Modes and exit codes

```
run.sh                  repo mode  — every check, whole tree. The gate.
run.sh --file <path>…   scoped     — file-addressable checks for those paths
run.sh --hook           PostToolUse — reads hook JSON on stdin, then --file
run.sh --stop           Stop hook  — repo mode behind two early-outs
run.sh --only a,b       run only the named checks (used by the test suite)
run.sh --list           print every check id, one per line
```

`0` clean · `1` violations · `2` violations in a hook mode · `3` misconfigured, which covers
a missing required tool (`jq` everywhere, ast-grep on rust) as well as a bad config.

The `2` is load-bearing, not stylistic: Claude Code ignores a `1` from a hook. Only `2`
shows stderr to the agent on `PostToolUse`, and only `2` blocks on `Stop`. A `--stop` that
exited `1` would leave the whole layer inert while looking correctly wired up.

## Changing a rule

Edit it **here**. A generated project's `scripts/guardrails/` is copied verbatim and never
hand-edited — change that project's `guardrails.config.json` instead. Stack differences are
data, not code; the module is the same file everywhere on purpose.

Then run the suites:

```bash
bash guardrails/__tests__/run-fixtures.sh    # bash checks, on real trees
bash guardrails/__tests__/run-plugin.sh      # the oxlint house plugin, real oxlint
bash guardrails/__tests__/run-latency.sh     # repo < 2000ms, --file < 250ms
```

The first two also run inside `scripts/validate-templates.sh`, which CI runs on every PR.
The latency suite is run by hand: a slow gate gets routed around, so the budget is measured
on a generated 500-file / 20-feature tree rather than on the fixtures. An over-budget sample
is retried twice and judged by the median, so one shared-runner scheduling pause cannot make
the gate flaky while a repeatable regression still fails. Workspace repo mode runs isolated
package checks concurrently, so its wall time tracks the slowest package rather than their sum.
Unexpected workspace-child exits are fatal, and a latency sample is eligible to pass only when
the measured guardrail command itself exits successfully.
