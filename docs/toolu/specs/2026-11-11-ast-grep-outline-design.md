# ast-grep outline — Design

**Date:** 2026-11-11   **Status:** Approved   **Author:** Claude (epic worker, Falconiere/toolu#857)   **Topic:** `toolu ast-grep outline` prints file items and directory maps

## Problem
Agents page through files to find items: 314 of 1,287 reads reopen a file at a new range (epic #853). One call that lists every item of a file with its line range, or every file of a directory with its public names, lets the next read be a range.

## Non-Goals
1. A cache, a repository-wide map, symbol lookup (issue non-goals).
2. Languages other than Rust, TypeScript/TSX, JavaScript/JSX and Python.
3. An item `kind` field: ast-grep's JSON does not report the node kind and the signature already shows it.
4. Changing quality-scan behavior.

## Architecture
- **Scan runner split** (`crates/core/engine/src/quality/scan.rs`): `binary(ctx)` becomes `pub fn binary(env: &Env, cwd: &Path) -> Option<PathBuf>`; the process part of `scan_inline` and the failure part of `parse` become `pub fn run_scan(env, cwd, rules, paths) -> Option<Result<String, ScanFailure>>` (`None` = no ast-grep on PATH; `Err` = timeout, output limit, nonzero exit, stderr, spawn error; same 8 MiB cap and messages). `scan_inline` calls `run_scan` and parses stdout as today. Quality behavior is byte-identical.
- **Core module `toolu_engine::outline`** (`outline.rs` + `outline/{rules,parse,text}.rs`):
  - `rules`: one inline YAML document per language — `rust`, `typescript`, `tsx`, `javascript`, `python`. The issue groups TypeScript/JavaScript as one; ast-grep binds each document to one parser, so TS, TSX and JS (with JSX) need their own document with the same rule body (JS without the TS-only kinds). Each matches item kinds with `has: {field: name, pattern: $NAME}` (the name comes from `metaVariables.single.NAME.text`) and `not: inside: <function-like kinds>, stopBy: end`, so methods stay and locals drop. Rust: fn, struct, enum, union, trait, impl (name = `type` field), mod, type, const, static, macro_rules, trait method signatures. TS/JS: function, generator function, class, abstract class, method, interface, type alias, enum, namespace, and top-level `const/let` declarators. Python: def, class.
  - `parse`: compact JSON matches into `(file, start, end, name)`.
  - `text`: from the file text derive the range start extended upward over contiguous doc and attribute lines (`///`, `//!`-free, `#[...]`, `/** … */`, `@decorator`), the signature (item lines from its first line, whitespace-collapsed, cut before the body opener `{`/`;` at bracket depth 0 for Rust/TS/JS, `:` at depth 0 for Python; 160 characters, `...` when cut), the first doc sentence (Rust `///`, JSDoc first non-tag line, Python docstring; cut at the first `. `; 100 characters), nesting depth by range containment, and `public` (Rust `pub…`, TS/JS line starts with `export`, Python name not starting with `_`).
  - **Batching (decision):** the issue settles "one process covers all files". Measured on this repo, ast-grep's JSON is ~4.7× the source (each match's text appears three times and containers repeat nested items): 150 files of `crates/epic-orchestrator/src` (800 KB) give 3.7 MB of JSON; directories averaging over ~11 KB per file would exceed the 8 MiB cap and fail entirely. So `outline` groups paths into batches of at most 1 MiB of source and runs one process per batch: one process for typical input, cap kept. Jev (choice, confidence 0.34) slightly preferred a single process; the measurement decides.
- **CLI** (`crates/ast-grep/src/cli.rs` `outline` verb, `crates/ast-grep/src/map.rs`): checks `binary()` first (exit 69), expands directories with `git ls-files -z --cached --others --exclude-standard -- <dir>` (filtered to supported extensions and existing files, sorted; only the first 150 are scanned, the rest counted as omitted), so `outline()` receives file paths only; calls `outline` once for all files from all arguments, renders per argument in order. `toolu-ast-grep` gains a path dependency on `toolu-engine` (rule → core is allowed by `layers.rs`).

## Interfaces / Schema
```rust
// toolu_engine::outline
pub struct Item { pub name: String, pub signature: String, pub doc: Option<String>,
                  pub start: usize, pub end: usize, pub depth: usize, pub public: bool } // 1-based lines
pub struct FileOutline { pub path: String, pub lines: usize, pub items: Vec<Item> }      // items by start
pub fn outline(env: &Env, cwd: &Path, paths: &[String]) -> Result<Vec<FileOutline>, String>;
pub fn enclosing(file: &FileOutline, line: usize) -> Option<&Item>; // innermost item covering line
pub fn supported(path: &str) -> bool;                                // extension check, CLI only
// toolu_engine::quality::scan
pub fn binary(env: &Env, cwd: &Path) -> Option<PathBuf>;
pub fn run_scan(env: &Env, cwd: &Path, rules: &str, paths: &[String]) -> Option<Result<String, ScanFailure>>;
```
CLI: `toolu ast-grep outline <path>... [--json]` (global `--json`).

File text (one block per file argument):
```
src/a.rs: 11 lines, 4 items
1-3 pub struct S // Doc one.
4-10 impl S
  5-5 pub fn new() -> Self
  6-9 fn two<T>(&self, x: T) where T: Clone
```
Two spaces of indent per depth; ` // <doc>` only when a doc exists. After 150 items: `... <n> more items`.

Directory text (one block per directory argument):
```
src/: 2 files
src/a.rs: 11 lines, 4 items: S
src/b.ts: 17 lines, 9 items: add, K, f, I, T
```
Names are depth-0 public items, at most 12, then `, +<n> more`; the `: names` part is dropped when there are none. After 150 files: `... <n> more files`. Blocks are separated by one blank line. When the text passes 12 KB (12,288 bytes) it stops at the last whole line and prints `... output truncated at 12 KB; outline fewer paths`.

`--json` prints one document (item and file caps apply; the 12 KB text cap does not):
```json
{"paths":[
  {"path":"src/a.rs","lines":11,"items":[{"name":"S","signature":"pub struct S","doc":"Doc one.","start":1,"end":3,"depth":0,"public":true}],"omitted":0},
  {"dir":"src/","files":[{"path":"src/a.rs","lines":11,"items":4,"names":["S"]}],"omitted":0}]}
```

## Failure modes and edge cases
- No `ast-grep` on PATH: stderr `ast-grep outline: ast-grep not found on PATH; install it with: brew install ast-grep (or cargo install ast-grep --locked)`, exit 69. `outline()` returns the same message as `Err`.
- No path argument: clap usage error, exit 64.
- Path does not exist: `ast-grep outline: <path>: no such file or directory`, exit 64.
- File with an unsupported extension given explicitly: `ast-grep outline: unsupported language: <path>`, exit 64. Inside a directory such files are skipped silently.
- Directory outside a Git work tree or `git ls-files` failure: `ast-grep outline: git ls-files failed: <first stderr line>`, exit 1.
- Directory with no supported files: `<dir>: 0 files`, exit 0.
- File with no items (empty, comments only): `<path>: <n> lines, 0 items`, exit 0.
- Unreadable file, ast-grep failure, timeout or output limit: `ast-grep outline: <reason>`, exit 1; no partial output.
- Files tracked but deleted from the work tree are skipped in directory expansion.
- Non-UTF-8 file: read lossily; ranges remain correct.

## Acceptance criteria
Issue criteria map as: issue AC 1 → AC-1, AC 2 → AC-2, AC 3 → AC-3, AC 4 → AC-4, AC 5 → AC-5; AC-6 covers the proposed `enclosing` interface. Every issue decision appears in Architecture (scan split, item rules, derived fields, caps, git expansion, exit 69); the only deviations are batching and per-parser documents, each with its reason.

- **AC-1:** A Rust file with a documented struct, an impl with two methods and a function holding a local function yields the struct, impl, both methods and the function with correct ranges, depths and names, and no local; the same for one TypeScript file (class with methods, function with inner function) and one Python file (class with methods, def with nested def).
- **AC-2:** A multi-line signature, a `where` clause, a doc comment above `#[derive]`, a JSDoc block and a Python docstring each render as one line: signature cut before the body, doc reduced to its first sentence, range starting at the first doc/attribute/decorator line.
- **AC-3:** `toolu ast-grep outline <file> <dir>` prints exactly the documented file and directory text; `--json` prints one parseable JSON document of the documented shape; with no `ast-grep` on PATH the exit code is 69 and stderr names the install command.
- **AC-4:** Quality scans behave as before: existing scan unit tests and the golden quality corpus pass unchanged.
- **AC-5:** `docs/cli/` is regenerated and the ast-grep skill and deep-explore agent tell agents to outline first and then read only the printed ranges; `cargo xtask check-markdown-cli` passes.
- **AC-6:** `enclosing(file, line)` returns the innermost item covering `line` (a method inside its impl) and `None` outside every item.

## Acceptance evidence
- **AC-1, AC-2, AC-6:** temp files written by `crates/core/engine/src/tests/outline_test.rs`, scanned by the real ast-grep; assert items (name, signature, doc, start, end, depth, public) and the absence of locals. Boundary: local fn inside a method, nested def in Python, decorator and attribute lines. Check: `cargo test -p toolu-engine outline`.
- **AC-3:** `crates/cli/tests/ast_grep_outline.rs` runs the built `toolu` in a real temp Git repo (`git init`, files added) with the real ast-grep on PATH; compares stdout byte-for-byte for a file and a directory argument; parses `--json` stdout with serde_json; runs with `PATH` set to a dir holding only `git` to get exit 69. Boundary: unsupported explicit file exits 64. Check: `cargo test -p toolu-cli --test ast_grep_outline`.
- **AC-4:** unchanged tests. Check: `cargo test -p toolu-engine quality::scan && cargo nextest run -p toolu-cli --test golden_quality`.
- **AC-5:** Check: `cargo xtask docs-cli && cargo xtask check-markdown-cli` and `git diff --exit-code docs/cli` after regeneration.

## Documentation impact
`plugins/ast-grep/skills/ast-grep/SKILL.md` (outline verb, outline-first reading), `plugins/toolu/agents/deep-explore.md` (outline first, then read the printed ranges), `docs/cli/` regenerated with `cargo xtask docs-cli` (including `commands.json`).

## Open Questions
None. Batching and the absent `kind` field are decided above with their reasons.

## Spec review
- Round 1: Jev coverage 0.35. Fixed: per-parser rule documents stated with reason, 150-file scan limit and file-only `outline()` input stated, issue-AC mapping added.
- Round 2: Jev coverage 0.59, least-covered choice `none` (0.66); buildable 0.89. Checklist: problem, non-goals, one decided architecture, failure exits, real-data evidence per AC, docs impact, no open questions — no blockers.

**Status:** Approved
