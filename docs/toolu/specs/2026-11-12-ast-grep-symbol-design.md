# ast-grep symbol — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Claude (epic worker, Falconiere/toolu#860)   **Topic:** `toolu ast-grep symbol` prints a name's definitions and references with their enclosing items

## Problem
To answer "where is `foo` defined and who calls it", agents run `grep`, then page through each hit's file to learn which function holds it. 27% of tool turns only read or search (epic #853). One call that returns the definition with its range and signature, and each reference with the item that contains it, lets the next read target a known range.

## Non-Goals
1. Type-aware resolution: a name shared by two items lists both (issue non-goal).
2. Languages beyond the ones `toolu_engine::outline` reads (Rust, TS/JS, Python) for definitions and enclosing items. Hits in other files appear only as plain references.
3. Changes to `toolu_engine::outline` or its `Item` type.

## Architecture
- **New module `crates/ast-grep/src/symbol.rs`** (and `tests/symbol_test.rs`), wired from `cli.rs` as verb `symbol`. Tests are split from rendering when the file nears 300 lines.
- **References:** `git grep -n -z -w -I -F --untracked -e <name> -- <pathspec>` in the current directory, through `toolu_runtime::process::Spec` with `max_output_bytes = 256 KiB`. Without paths the pathspec is the outlined extensions (`*.rs *.ts *.mts *.cts *.tsx *.js *.mjs *.cjs *.jsx *.py *.pyi`, the same set as `outline::supported`); given paths replace it. Exit 1 from git means no match. The output is parsed as `references` parses it (`crates/code-review/src/review/context.rs:148`): `path\0line\0text` per line. The parsing is copied, because a rule crate may not use the code-review crate (`crates/xtask/src/layers.rs:103`).
  - `--untracked` (decision): it matches the file set of `outline <dir>` (tracked plus untracked, not ignored), and agents look up names in files they have just written. Jev (noul 0.78) agreed.
- **Definitions:** files with hits that `outline::supported` accepts are sorted, and the first 150 are passed to one `outline(env, cwd, files)` call. A hit at line L is a definition when an item named `<name>` covers L and the hit line, trimmed and whitespace-collapsed, agrees character by character with the start of that item's signature (a trailing `...` cap is dropped first; one string must be a prefix of the other). This is the issue's "hit on an item's own signature line", worked out from the existing `Item` fields, so the core type does not change. An `impl Foo` block counts as a definition of `Foo` (decision: no special case, matches "lists both"; Jev choice `def` 0.93). Every other hit reports `outline::enclosing(file, L)`.
- **Without ast-grep** (`toolu_engine::quality::binary` is `None`): no outline. All hits print as references without enclosing items, followed by one note. Exit 0 when there are hits.
- **Caps:** read at most 256 KiB of grep output; outline at most 150 files; print at most 10 definitions and 40 references, at most 8 per file. Each printed text line is cut to 120 characters (`...` when cut). Past any cap, the totals line still shows the full counts, and a final line reads `Narrow: toolu ast-grep symbol <name> <path>`.

## Interfaces / Schema
CLI: `toolu ast-grep symbol <NAME> [PATH]... [--json]` (global `--json`).

Text:
```
add: 1 definition, 3 references in 3 files
def src/a.rs:1-5 pub fn add(x: i32) -> i32
src/b.rs
  4 in main 2-6: let y = add(1);
src/c.ts
  1: add(2);
```
- The header is `<name>: <d> definition(s), <r> reference(s) in <f> file(s)`, with plain plural `s` when the count is not 1. It counts every parsed hit; definitions and references are disjoint.
- Definition lines read `def <path>:<start>-<end> <signature>`, ordered by path and then line.
- References are grouped under a file header line (the bare path) and ordered by path and then line. Each reads `  <line> in <item> <start>-<end>: <text>`, or `  <line>: <text>` when no item encloses the line (top level, an unsupported file, an un-outlined file, or no ast-grep). `<text>` is the trimmed source line.
- Past the 8-per-file cap a file adds `  ... <n> more in this file`. Past the 40 or 10 cap, the remaining files and definitions are not printed.
- Optional trailing lines, in this order: the one note, `note: ast-grep not found on PATH; definitions and enclosing items are skipped (install: brew install ast-grep)`, printed only without ast-grep; then `Narrow: toolu ast-grep symbol <name> <path>`, printed when any cap dropped output. No other note exists.
- When grep output passed 256 KiB, the header counts are partial and the header ends with ` (grep output capped at 256 KiB)`. Hits in files past the 150-file outline cap print without an `in` part.
- Every printed line (header, `def`, file, reference) is cut to 120 characters, ending in `...` when cut.

`--json`: one document, with the same caps:
```json
{"name":"add","definitions":[{"path":"src/a.rs","line":2,"start":1,"end":5,"signature":"pub fn add(x: i32) -> i32"}],
 "references":[{"path":"src/b.rs","line":4,"text":"let y = add(1);","in":{"name":"main","start":2,"end":6}}],
 "totals":{"definitions":1,"references":3,"files":3},"partial":false,"truncated":false,"note":null}
```
`in` is `null` when no item encloses the line. `partial` is true when grep output passed 256 KiB. `truncated` is true when any cap dropped output. `note` is the ast-grep note or `null`. JSON strings are not cut to 120 characters.

Exit codes: 0 when there are hits; 1 when there is no match (`ast-grep symbol: no match for <name>` on stderr), git fails (its first stderr line), or the outline fails; 64 for an empty name.

## Failure modes and edge cases
- **Empty name:** exit 64 `ast-grep symbol: name must not be empty`. A name that begins with `-` is safe, because it is passed after `-e`.
- **Not a git repository / git fails:** exit 1 with git's first stderr line. A timeout also exits 1.
- **No match** (git exit 1, or only partial lines parsed): exit 1.
- **Grep output over 256 KiB:** keep the whole lines read so far (the partial last line is dropped), mark the header partial, print `Narrow:`, and exit 0.
- **More than 150 hit files:** the files after the first 150 in path order are not outlined; their hits are plain references, and `Narrow:` is printed.
- **Hits in unsupported files** (`.md`, given paths): plain references without an item.
- **Path that does not exist:** git matches nothing, so exit 1 "no match".
- **Outline error** (ast-grep fails): exit 1 `ast-grep symbol: <error>`. A file too large to outline is left out by `outline`, and its hits stay plain references.
- **Binary files:** `-I` skips them. Non-UTF-8 text is decoded lossily.
- **Paths containing `:`:** handled, because `-z` separates the fields with NUL.

## Acceptance criteria
- **AC-1:** Given a git repo where `add` is defined once in `src/a.rs` and called in three other files, `toolu ast-grep symbol add` prints exactly one `def src/a.rs:<start>-<end> <signature>` line, and each call under its file as `<line> in <item> <start>-<end>: <text>`. Exit 0.
- **AC-2:** Given 60 call sites of a name spread over files with more than 8 calls each, the output prints exactly 40 reference lines, at most 8 under any file, the full totals in the header, and the `Narrow:` line.
- **AC-3:** An unknown name exits 1 with empty stdout. With only `git` on PATH (no ast-grep), the references still print, with no `in` part, plus the ast-grep note. Exit 0.
- **AC-4:** `--json` prints one document with `definitions`, `references` (`in` object or null), `totals`, `partial`, `truncated` and `note`.
- **AC-5:** Agent text (`plugins/ast-grep/skills/ast-grep/SKILL.md`) and `docs/cli/` match the binary.

## Acceptance evidence
- AC-1: `crates/cli/tests/ast_grep_symbol.rs` builds a real temp git repo (Rust and TS files, one call at top level) and runs the real binary with real ast-grep. It asserts byte-exact stdout. Boundary: a top-level call prints without `in`, and a `.md` mention prints as a plain reference. Check: `cargo test -p toolu-cli --test ast_grep_symbol`.
- AC-2: the same target with a generated repo of 6 files × 10 calls = 60. It asserts 40 reference lines, ≤ 8 per file, the header `... 60 references in 7 files`, and the last line `Narrow: toolu ast-grep symbol <name> <path>`.
- AC-3: the same target. Unknown name: exit 1, stdout empty, stderr `ast-grep symbol: no match for nope`. With a PATH holding only a `git` symlink: exit 0, references with no `in`, and the single note line.
- AC-4: the same target, parsing stdout as JSON.
- Unit tests in `crates/ast-grep/src/tests/symbol_test.rs` cover grep parsing (NUL fields, partial last line), the signature-line match (multi-line signature, `...` cap, doc line), the per-file and total caps, and the 120-character cut, using real `FileOutline` values.
- AC-5: `cargo xtask docs-cli && cargo xtask check-markdown-cli`, plus `cargo test -p toolu-cli --test contract` (snapshot).

## Documentation impact
- `plugins/ast-grep/skills/ast-grep/SKILL.md`: add `symbol` next to `outline` ("where is X defined / who uses it").
- `docs/cli/ast-grep.md` and `docs/cli/commands.json`: regenerated by `cargo xtask docs-cli`. The contract snapshot `crates/cli/tests/fixtures/commands_json.snap` is updated.

## Open Questions
None. Two decisions are recorded above: `--untracked`, and impl blocks counting as definitions.

## Review
- Failure modes: 🟡 should-fix (fixed): extra notes contradicted the issue's "one note". Only the ast-grep note remains; partial grep output is marked in the header.
- Architecture: 🔵 consider (kept): `--untracked` widens "the working tree" to untracked files that are not ignored. They are part of the working tree, and the sibling `outline` command uses the same set. Jev's gap check was diffuse (0.34 untracked, 0.23 none).

**Status:** Approved
