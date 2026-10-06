# Brainstorm — Markdown–CLI drift gate (#444)

Delivery mode, Full path: a new required check over every plugin's Markdown,
with an allowlist format and CI placement that later ports (#421, #425–#453)
build on.

## Capsule

- **Outcome:** `cargo xtask check-markdown-cli` reads `docs/cli/commands.json`
  and fails when a scanned Markdown file names a `toolu` command, verb, flag,
  flag value or positional count the tree lacks, or tells the agent to run a
  removed surface (a `hooks/dist/` bundle, a `scripts/*.ts`, a stable script)
  whose namespace is ported. Findings name the file, line, the offending
  token and the closest valid verb or the valid flags. It runs in
  `bun run test` and `bun run test:docs`, in milliseconds.
- **Material defaults / non-goals:** the tree comes from the committed
  `docs/cli/commands.json`, not a fresh binary build (the gate's `docs-cli`
  step already proves the two equal). Dated design records (`docs/toolu/**`)
  and old release notes (`docs/releases/**`) are not scanned. Not part of
  `cargo xtask gate` yet; #439 moves it there. No Markdown is rewritten to the
  `toolu` CLI here; each port does that for its namespace.
- **Repository evidence:** `crates/xtask/src/cli_compat/compare.rs` already
  walks the same JSON tree (children by name or alias, flags by `long`,
  `placeholder` verbs). `docs/cli/commands.json` lists globals on the root
  with `global: true`, `--version`/`-V` and the hidden `--hook-protocol` as
  root flags, and a hidden `hook` verb on every plugin namespace. Today no
  plugin Markdown names `toolu …`; it runs `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js"`,
  `"$S/launch-issue.ts"`, `jev.sh` and `write-state.sh`.
- **Risk:** inline code spans in prose are mentions as often as commands; a
  too-strict reading fails correct docs (`toolu hook` takes a fast path). The
  `docs` and `gate` CI jobs gain a Rust toolchain.
- **Handoff:** spec.

## Axes

### Where it runs in CI (Jev: `gate_only_plus_paths` 0.71, `gate_and_bun` 0.22, `bun_only` 0.07)

Every trigger reaches the `docs` group already: a verb rename regenerates
`docs/cli/**` (`docs/**`), and every scanned surface is `**/*.md`. So
`bun run test:docs` alone catches the issue's scenario. The issue also names
`bun run test`. Decision: add `cargo xtask check-markdown-cli` to `test:docs`
and `test:ts`, and install the pinned toolchain in the `docs` and `gate` jobs.
Jev preferred adding the Markdown paths to the `rust` group, but that runs the
full Rust gate on two platforms plus two musl builds for every skill edit and
contradicts the issue's "part of `bun run test` until #439". Overridden on
cost and on the issue text.

### Allowlist location (Jev: `in_plugin` 0.56, `in_tooling` 0.44)

`tooling/conventions/markdown-cli/<plugin>.json` per plugin (execution moved it
out of `plugins/<name>/`: the plugin folder allowlist is gate data),
and `tooling/conventions/markdown-cli.json` repository-wide (spec review moved it out of `docs/`). A file is
optional (absent = no entries). Each entry names `file`, the exact `subject`
text and a non-empty `reason`; an entry that no longer matches a finding fails
as stale, so an allowance cannot outlive the wrong usage it excuses. Two real
entries today: `AGENTS.md`'s `toolu install` (the Node installer's bin, also
named `toolu`, until #438) and `docs/resource-budgets.md`'s
`toolu statusline render` (the budget for the verb #431 adds).

### Scope of `docs/**` (Jev: `exclude_records` 0.96)

Scan `docs/**/*.md` except `docs/toolu/**` (dated brainstorms, specs and
plans; they quote future verbs and deliberate typos as scenarios) and
`docs/releases/**`. The exclusion is fixed in the task source, not a per-path
override field in data.

### Inline code versus fenced shell (Jev: `mention_tolerant_inline` 0.90)

Both check every command name, verb, flag, flag value and an excess of
positionals. A fenced shell line must be complete: a verb where the tree needs
one, and every required positional. An inline span may stop at a namespace
(`toolu doctor`) or omit required positionals (`toolu hook`), because prose
names commands.

### What counts as an invocation

- Fenced blocks tagged `bash`, `sh`, `shell`, `console`, `zsh`, `fish` or
  untagged; a command segment (after `$ `, `&&`, `||`, `|`, `;`, `$(`) that
  starts with `toolu`. Backslash continuations join.
- Inline spans whose text is `toolu` followed by a command-like token: a
  lowercase word, a flag, or a placeholder. `toolu PostToolUse dispatcher failed`
  and `toolu runtime: native …` are messages, not commands.
- Placeholders (`<ref>`, `[<plugin>]`, `$VAR`, `"$VAR"`, `${…}`) are values. In a
  command position a placeholder matches any child (`[…]` may also be
  absent); `…`/`...` ends the check.
- `--help`/`-h` are accepted everywhere (clap adds them; the tree omits them).

### Removed surfaces

Checked in plugin Markdown only, where the text instructs the agent;
`AGENTS.md` and `docs/**` still describe the TypeScript tree, which exists
until #440. A reference is a `hooks/dist/<stem>.js` or `scripts/<stem>.ts` path,
a bare `bun` (or `"$…BUN…"`) running a `.ts`/`.js` file, or a stable script
from a fixed list (`jev.sh`, `write-state.sh`, `search.sh`, `statusline.sh`).
Its namespace comes from a stem table (`plan-ledger`/`verdict` → `ledger`,
`jev` → `jev`, `write-state` → `review`, …), falling back to the namespaces
the containing plugin owns. It fails once that namespace is ported: present,
with no `placeholder` verb. `brainstorm` and `delivery-flow` already count as
ported, which is why their `plan-ledger.js` references must map to `ledger`.

## Rejected alternatives

- **Building and running `toolu commands --json`:** minutes of release build
  in a check budgeted at 5 s, and redundant with the `docs-cli` step.
- **A TypeScript checker:** the issue names `cargo xtask`, and #439 deletes the
  TypeScript tooling.
- **Allowlist keyed by line number:** drifts with every edit above it.
- **Scanning every `docs/**`:** dozens of allowlist entries for historical
  records that are not instructions.

## Revision at spec review

The issue's acceptance "an external command on the allow-list, such as
`ast-grep run`, passes; the same command off the list fails" means every
command in a shell-tagged fence is judged, not only `toolu` lines (Jev 0.90).
The allowlist files gain an `external` list of command names; shell builtins,
keywords, functions defined in the block, paths and variables pass on their
own. Untagged and `text` fences hold output and trees, so they are not
scanned. The first real finding is the ast-grep skill's stale `mod.sh`
wrapper name.
