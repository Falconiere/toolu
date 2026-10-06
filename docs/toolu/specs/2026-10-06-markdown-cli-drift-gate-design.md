# Markdown–CLI drift gate — Design

**Date:** 2026-10-06   **Status:** Approved   **Author:** Claude (epic #402 worker)   **Topic:** #444, fail CI when Markdown names a `toolu` command, verb or flag the CLI lacks

## Problem

Skills, commands and agents are the CLI's user interface: they tell the agent
which `toolu …` to run. A renamed verb or removed flag silently breaks them,
and the break shows up only in a live session. As each namespace is ported
(#421, #425–#453), Markdown moves from `bun …/hooks/dist/*.js` to `toolu …`,
and nothing checks that the two agree. The brainstorm
(`docs/toolu/brainstorms/2026-10-06-markdown-cli-drift-gate.md`) settled the
design forks; this spec is the contract.

## Non-Goals

1. Rewriting Markdown to `toolu …`; each port does that for its namespace.
   The one Markdown fix here is a stale wrapper name the new check finds
   (`mod.sh ast-grep …` in the ast-grep skill's advanced reference).
2. Building or running the `toolu` binary. The tree is the committed
   `docs/cli/commands.json`; the gate's `docs-cli` step proves it current.
3. Adding the check to `cargo xtask gate`; #439 moves it there.
4. Checking `toolu` invocations in code, `hooks.json` (`cargo xtask check-hooks`
   owns those), `docs/toolu/**` or `docs/releases/**` (dated records).
5. Judging an external command's own arguments (`gh`, `git`, `ast-grep`).
6. Judging untagged or non-shell fences (`text`, `json`, …): they hold
   output, trees and data, not commands.

## Architecture

A new task `check-markdown-cli` in `crates/xtask`, module `markdown_cli` with
submodules, each with a co-located `tests/<module>_test.rs` (rules 6–7) and
under 300 code lines:

- `markdown_cli.rs` — `run`: load the tree and allowlists, list files, scan,
  judge, apply allowances, print through `output::findings`.
- `markdown_cli/scan.rs` — Markdown → shell-tagged fenced blocks (`bash`,
  `sh`, `shell`, `zsh`, `fish`, `console`) with their first line number, and
  single-backtick inline code spans outside fences.
- `markdown_cli/shell.rs` — a small shell lexer over a whole block: words with
  their line, quotes (single, double; a quoted word may span lines),
  backslash-newline joins, `#` comments, heredoc bodies skipped (`<<X`,
  `<<'X'`, `<<"X"`, `<<-X`), redirections dropped with their target, and
  command boundaries at newline, `;`, `&`, `&&`, `||`, `|`, `(`, `)`, `{`,
  `}`, `$(` and the keywords `if then elif else while until do ! time`. It
  yields `Command { line, words }` after leading `NAME=value` assignments; a
  `console` block keeps only lines starting `$ `. A `for NAME in …` header is
  not a command; `name()` and `function name` record a defined function.
- `markdown_cli/judge.rs` — a `toolu` command against the tree JSON. Children
  by `name` or `aliases` (hidden children match but are never suggested);
  flags from the current node plus each ancestor's `global: true` flags, plus
  `--help`/`-h` everywhere; `--flag=value`; a value flag consumes the next
  word; listed `possibleValues` are enforced unless the value is a
  placeholder (`<…>`, `[…]`, `$…`, `${…}`). A placeholder in command position
  tries every child (a `[…]` one may also be absent); `…`/`...` accepts the
  rest. Positionals beyond the node's `args` fail unless the last is
  `multiple`. A fenced command must end on a node without children and give
  every required arg; an inline span may stop at a namespace or omit
  required args (prose mentions). Closest verb: Levenshtein distance over
  visible child names and aliases, ties by tree order.
- `markdown_cli/names.rs` — the command name of every other fenced command:
  passes when it is a shell builtin or keyword (fixed list in the source), a
  function defined in the same block, a path (contains `/`) or a variable
  (`$…`), or listed in `external`; otherwise a finding.
- `markdown_cli/surfaces.rs` — removed surfaces, in plugin Markdown only
  (fences and inline spans): a word containing `hooks/dist/<stem>.js` or
  `scripts/<stem>.ts`, a `bun`/`$…BUN…` command running a `.ts`/`.js` file,
  and the stable scripts `jev.sh`, `write-state.sh`, `search.sh`,
  `statusline.sh`. A stem table maps a stem to its namespace (`plan-ledger`,
  `verdict` → `ledger`; `jev` → `jev`; `write-state` → `review`;
  `babysit-*` → `babysit`; `debug-*` → `debug`; `setup` → `setup`; `status`,
  `statusline` → `statusline`; `search`, `ast-grep` → `ast-grep`); any other
  stem maps to the namespaces whose `owner` is the containing plugin. A
  reference fails when every namespace it maps to exists and has no
  `placeholder` verb ("ported").
- `markdown_cli/allow.rs` — loads `tooling/conventions/markdown-cli.json`
  (repository-wide) and `plugins/<name>/markdown-cli.json` (that plugin's
  files only), validates them, matches findings and reports stale entries.

Reuse: `output::findings`/`say`, `Options.root`, `serde_json::Value` as in
`cli_compat/compare.rs`; no new dependency.

Wiring: `"check:markdown-cli": "cargo xtask check-markdown-cli"` in
`package.json`, run by `test:ts` and `test:docs`; `.github/workflows/tests.yml`
installs the pinned toolchain (`rustup toolchain install`) and
`Swatinem/rust-cache` in the `gate` and `docs` jobs. `TASKS`, `USAGE` and
`inventory.json` gain the task. Every trigger already reaches the `docs`
group: a verb rename regenerates `docs/cli/**`, and every scanned file is
`**/*.md`.

## Interfaces / Schema

`cargo xtask check-markdown-cli [--root DIR]`. Exit 0 clean
(`check-markdown-cli: ok`), 1 findings (one per line on stderr, then the
count), 2 setup error.

Scanned: `plugins/*/skills/**/*.md`, `plugins/*/commands/*.md`,
`plugins/*/agents/*.md`, `AGENTS.md`, `docs/**/*.md` except `docs/toolu/**`
and `docs/releases/**`.

Finding lines (`<file>:<line>: <subject>: <problem>`):

```text
plugins/epic-orchestrator/skills/x/SKILL.md:12: `toolu epic strat`: unknown command `strat` under `toolu epic` (closest: `planned`; valid: planned)
plugins/jev/skills/x/SKILL.md:8: `toolu --jsn epic planned`: unknown flag `--jsn` on `toolu` (valid: --json, --quiet/-q, --host, --config-dir, --version/-V, --help/-h)
plugins/ast-grep/skills/ast-grep/references/ast-grep-advanced.md:28: `mod.sh`: not toolu and not on the external allow-list
plugins/delivery-flow/skills/delivery-flow/references/execution.md:22: `hooks/dist/plan-ledger.js`: a removed surface; `toolu ledger` is ported
tooling/conventions/markdown-cli.json: stale allowance `toolu install` in AGENTS.md: it no longer fails
```

Allowlist file (both locations):

```json
{
  "external": ["gh", "git", "ast-grep"],
  "allow": [{ "file": "AGENTS.md", "subject": "toolu install", "reason": "…" }]
}
```

Both keys optional; unknown keys rejected. `external` names commands that are
not `toolu`. An `allow` entry excuses the finding whose file and subject (the
backticked text) match exactly; `file` is repository-relative and must lie in
the file's scope (the plugin directory, or anywhere scanned for the
repository-wide file); `reason` is non-empty. Initial repository-wide data:
`external` = every external command today's Markdown runs (`bun`, `npx`,
`ast-grep`, `cat`, `rm`, `codex`, `claude`, `opencode`, `git`, `gh`, `jq`,
`curl`, `brew`, `comemory`, `grep`, `cp`, `herdr`, `cargo`, `npm`, …, exactly
those the scan meets), and `allow` = `AGENTS.md` `toolu install` (the Node
installer's bin, also named `toolu`, until #438) and
`docs/resource-budgets.md` `toolu statusline render` (the budget for the verb
#431 adds).

## Failure modes and edge cases

- `docs/cli/commands.json` absent or not JSON → exit 2 naming it and
  `cargo xtask docs-cli`.
- An allowlist that is not JSON, has an unknown key, an empty `reason` or a
  `file` outside its scope → exit 2 naming the allowlist.
- An `allow` entry that matches no finding → a stale finding, exit 1. An
  `external` name no scanned file runs is not a finding (the list is a
  vocabulary, not an exemption).
- An unreadable Markdown file → exit 2 naming it.
- An unterminated fence runs to the end of the file (CommonMark); an
  unterminated quote or heredoc runs to the end of the block.
- `toolu` followed by a capitalised word, a word with `:`, or nothing in an
  inline span → not an invocation (`toolu PostToolUse dispatcher failed`,
  `toolu runtime: …`). `@toolu/plugins` and `toolu-review` are other words.
- Several invocations on one line → each judged; findings sorted and
  deduplicated by file, line and text.

## Acceptance criteria

- **AC-1:** Given a skill (temp root with the real tree) whose line 12 is a
  fenced `toolu epic strat`, the task exits 1 and stderr names the file, line
  12, `strat` and the closest valid verb; the same text in an inline span
  fails the same way.
- **AC-2:** Given `toolu --jsn epic planned`, it exits 1 and lists the valid
  flags including `--json`; `--host bogus` fails and `--host <h>` passes.
- **AC-3:** On this repository the task exits 0, with today's mentions
  (`toolu hook`, `toolu doctor`, `toolu <plugin> hook <name>`,
  `toolu [<plugin>] hook <name> --event <Event> --plugin-root <root>`,
  `toolu --hook-protocol`) passing; unported namespaces are checked against
  `planned` (a fenced `toolu ledger run` fails).
- **AC-4:** `plugins/delivery-flow` and `plugins/brainstorm` are scanned (the
  file list of this repository contains their real `SKILL.md` files): with a
  tree in which `ledger` is ported, delivery-flow's real `execution.md` fails
  on `hooks/dist/plan-ledger.js`; with the real tree it passes.
- **AC-5:** A fenced `ast-grep run --pattern x` passes with the real external
  list and fails, naming `ast-grep`, when `ast-grep` is removed from the list.
- **AC-6:** Given a tree in which `babysit` has a verb `tick` and a skill runs
  `toolu babysit tick`, the task passes; after renaming the verb to `step` in
  the tree only, it fails naming the skill's file and line. In CI, a diff of
  only `crates/**` and `docs/cli/commands.json` (what a rename produces) turns
  on the `docs` group, whose `bun run test:docs` runs the task.
- **AC-7:** An `allow` entry excuses exactly its finding; a stale entry fails
  (exit 1); an entry without a reason or outside its scope is a setup error
  (exit 2).
- **AC-8:** The task finishes on this repository in under 5 s, measured
  around the spawned binary; a warm `cargo xtask check-markdown-cli` (cargo's
  no-op build check included) also finishes in under 5 s. A cold build is
  compilation, shared with every other xtask task, not the gate.
- **AC-9:** `bun run test:docs` and `bun run test` run the task, CI's `docs`
  and `gate` jobs install the toolchain, and `bun run check:ci-paths` passes.

## Acceptance evidence

| AC | Real input | Observable result | Boundary / failure | Check |
|---|---|---|---|---|
| AC-1 | temp root: real `docs/cli/commands.json` + `plugins/epic-orchestrator/skills/x/SKILL.md` | exit 1; stderr has `SKILL.md:12:`, `strat`, `closest: \`planned\`` | inline vs fenced | `cargo test -p xtask --test markdown_cli` |
| AC-2 | same root, flags | exit 1 with `--json` listed; `--host bogus` 1; `--host <h>` 0 | `--host=<h>` | same |
| AC-3 | this repository | exit 0, `check-markdown-cli: ok` | fenced `toolu ledger run` in a temp root → 1 | `cargo xtask check-markdown-cli`; integration test |
| AC-4 | real `plugins/delivery-flow/skills/delivery-flow/references/execution.md` in a temp root with `ledger`'s `planned` replaced by a real verb; this repository's file list | exit 1 naming `plan-ledger.js` and its line; the list holds `plugins/brainstorm/skills/brainstorm/SKILL.md` and delivery-flow's | real tree → 0 | integration test; `markdown_cli_test.rs` |
| AC-5 | temp skill with a fenced `ast-grep run --pattern x`; real `tooling/conventions/markdown-cli.json` copied, then with `ast-grep` removed | 0, then 1 naming `ast-grep` | shell builtins, functions, paths, variables pass | integration + `names_test.rs` |
| AC-6 | real tree edited in the test: `babysit`'s `planned` → `tick`, then `step`; the real `.github/ci-paths.json` classifying `crates/toolu/src/lib.rs` + `docs/cli/commands.json` | 0, then 1 naming the skill line; `docs` group on | — | integration test; `tooling/src/ci-paths/__tests__/changes.test.ts` |
| AC-7 | temp allowlists | excused → 0; stale → 1; no reason / wrong scope / unknown key → 2 | plugin file cannot excuse another plugin | integration + `allow_test.rs` |
| AC-8 | this repository | elapsed < 5 s | warm cargo invocation | integration test timing the spawned binary; `time cargo xtask check-markdown-cli` in the execution log |
| AC-9 | `package.json`, `tests.yml` | scripts and steps present | `bun run check:ci-paths` green | `bun run test:docs`, `bun run check:ci-paths` |

## Documentation impact

- New `docs/markdown-cli.md`: what is scanned, what counts as a command, the
  removed-surface rule and stem table, the allowlist format, and what a port
  does (rewrite its Markdown, extend the stem table).
- `AGENTS.md`: the `gate`/`docs` CI rows, the xtask task list in Key files,
  the Rust conventions Commands bullet, and Contributing.
- `plugins/ast-grep/skills/ast-grep/references/ast-grep-advanced.md`: the
  stale `mod.sh ast-grep …` wrapper name becomes the real
  `plugins/ast-grep/hooks/dist/ast-grep.js …`, as the skill's other examples.

## Open Questions

None blocking. Two recorded overrides: CI placement (Jev preferred the `rust`
path group; rejected on cost and the issue's "part of `bun run test` until
#439") and the untagged-fence exclusion (non-goal 6).

## Spec review

- Acceptance criteria: 🔴 blocker (fixed): the first draft checked only `toolu`
  lines, so the issue's "the same command off the list fails" could not be
  demonstrated. Added the `external` list and the command-name check (Jev 0.90
  that the issue requires it).
- Acceptance evidence: 🟡 should-fix (fixed): the rename scenario stopped at the
  task; AC-6 now proves the `docs` CI group fires on a rename diff.
- Acceptance evidence: 🟡 should-fix (fixed): `plugins/brainstorm` was not
  shown to be scanned; AC-4 asserts the real file list.
- Acceptance criteria: 🔵 consider (fixed): AC-8 now also times the warm cargo
  invocation.
- Jev alignment (`covered`): 0.73 → 0.74 after fixes; the remaining doubt is the
  meaning of "under 5 s", settled in AC-8.
