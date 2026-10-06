# Markdown–CLI drift gate — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-markdown-cli-drift-gate-design.md   **Topic:** #444 — `cargo xtask check-markdown-cli` judges every `toolu` invocation and every fenced shell command in plugin Markdown, `AGENTS.md` and live docs against `docs/cli/commands.json`.

## Evidence and approach

- **Inspected.** `crates/xtask/src/main.rs` (`TASKS`, `USAGE`), `options.rs` (`--root`), `output.rs` (`findings`), `cli_compat.rs` and `cli_compat/compare.rs` (walking the same JSON: children by name or alias, flags by `long`, `placeholder`), `tooling/conventions/guardrails/rust/inventory.json` (an `xtask-task` entry names a passing and a failing test), `docs/cli/commands.json` (root globals `json`, `quiet/q`, `host` with values, `config-dir`; root `version/V`, hidden `hook-protocol`; hidden `hook` verb on each plugin namespace; `owner` is the plugin directory), `.github/ci-paths.json` (`docs` = `docs/**`, `**/*.md`), `.github/workflows/tests.yml` (`gate` and `docs` jobs have Bun, no Rust), `tooling/src/ci-paths/__tests__/changes.test.ts` (real `ci-changes.ts` runs in git sandboxes), `package.json` (`test:ts`, `test:docs`).
- **Survey of real Markdown.** 98 scanned files; 71 `bash` fences; first words include `bun`, `ast-grep`, `npx`, `cat`, `rm`, `codex`, `git`, `gh`, `jq`, `curl`, `brew`, `comemory`, `opencode`, `claude`, `cp`, `grep`, shell keywords, assignments, a `judge()` function and the stale `mod.sh`. No plugin Markdown names `toolu …` yet; live docs name `toolu hook`, `toolu doctor`, `toolu <plugin> hook <name>`, `toolu statusline render`, `toolu install`.
- **Recalled.** Comemory `be52369e`: on this root host `bun run test` has environmental failures that also fail on `origin/main`; baseline against `origin/main` before calling a failure a regression; CI is the authority.
- **Toolchain.** Cargo 1.99 is in `~/.cargo/bin`; every check exports it. The worktree has no `node_modules`: run `bun install --frozen-lockfile` before S7. Gate and test runs go through the epic job lease (`job.ts`). Stage new files before checks that read tracked files.
- **Design.** As the approved spec: modules `markdown_cli/{scan,shell,judge,names,surfaces,allow}.rs` under `markdown_cli.rs`, each under 300 code lines with `markdown_cli/tests/<module>_test.rs`; integration tests in `crates/xtask/tests/markdown_cli.rs` spawn the real `xtask` binary on temp roots built from the real tree and real Markdown.

## Workstream summary

Shell lexer → Markdown scan → tree judge → command names and removed surfaces → allowlists → task wiring, data file, real-repo pass and integration tests → `package.json`/CI wiring with the path-group test → docs → full gates.

## Steps (machine-readable)

```json
[
  {
    "id": "S1-shell-lexer",
    "title": "markdown_cli/shell.rs: lex a shell block into commands (quotes spanning lines, backslash-newline, comments, heredoc bodies skipped, redirections dropped, separators, keywords, leading assignments, for-headers, defined functions, console $-prompt lines)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked markdown_cli::shell && cargo test -p xtask --locked markdown_cli::words && cargo test -p xtask --locked markdown_cli::shell::",
    "ac_refs": [
      "AC-5"
    ],
    "paths": [
      "crates/xtask/src/main.rs",
      "crates/xtask/src/markdown_cli.rs",
      "crates/xtask/src/markdown_cli/shell.rs",
      "crates/xtask/src/markdown_cli/tests/shell_test.rs",
      "crates/xtask/src/markdown_cli/words.rs",
      "crates/xtask/src/markdown_cli/tests/words_test.rs",
      "crates/xtask/src/markdown_cli/shell",
      "crates/xtask/src/markdown_cli/shell/tests"
    ],
    "input": "real blocks copied from plugins/jev/skills/jev/SKILL.md (the JEV_BUN for-loop), plugins/ast-grep/skills/ast-grep/references/ast-grep-advanced.md (multi-line single-quoted YAML), plugins/jev/skills/jev/references/problem-solving.md (cat <<'JSON' heredoc, judge() function); an unterminated quote and heredoc",
    "model": "inherit"
  },
  {
    "id": "S2-markdown-scan",
    "title": "markdown_cli/scan.rs: shell-tagged fences (bash, sh, shell, zsh, fish, console) with line numbers, single-backtick inline spans outside fences, untagged/text fences skipped, unterminated fence to EOF; the scanned file list (plugins skills/commands/agents, AGENTS.md, docs/** minus docs/toolu and docs/releases)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked markdown_cli::scan",
    "ac_refs": [
      "AC-3",
      "AC-4"
    ],
    "depends_on": [
      "S1-shell-lexer"
    ],
    "paths": [
      "crates/xtask/src/markdown_cli/scan.rs",
      "crates/xtask/src/markdown_cli/tests/scan_test.rs"
    ],
    "input": "real docs/install.md and docs/resource-budgets.md inline spans; a Markdown string with ```text, ``` untagged, ~~~bash and an unterminated fence; this repository's file list containing plugins/brainstorm/skills/brainstorm/SKILL.md and plugins/delivery-flow/skills/delivery-flow/SKILL.md and no docs/toolu file",
    "model": "inherit"
  },
  {
    "id": "S3-tree-judge",
    "title": "markdown_cli/judge.rs: walk words against the real tree (children and aliases, hidden matches not suggested, inherited globals, --help/-h, --flag=value, value flags, possibleValues unless placeholder, placeholder command positions incl. optional [..], ellipsis, positional count, fenced completeness vs inline mentions, Levenshtein closest verb)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked markdown_cli::judge",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3"
    ],
    "depends_on": [
      "S1-shell-lexer"
    ],
    "paths": [
      "crates/xtask/src/markdown_cli/judge.rs",
      "crates/xtask/src/markdown_cli/tests/judge_test.rs",
      "docs/cli/commands.json",
      "crates/xtask/src/command_tree.rs",
      "crates/xtask/src/tests/command_tree_test.rs",
      "crates/xtask/src/cli_compat/compare.rs"
    ],
    "input": "the real docs/cli/commands.json: toolu epic strat, toolu --jsn epic planned, toolu --host bogus commands, toolu --host <h> commands, toolu --host=<h> commands, toolu hook (inline ok, fenced missing NAME), toolu doctor (inline ok, fenced missing verb), toolu <plugin> hook <name>, toolu [<plugin>] hook <name> --event <Event> --plugin-root <root>, toolu --hook-protocol, toolu ledger run, toolu review hook a b, toolu --version, toolu jev planned --help, toolu …",
    "model": "inherit"
  },
  {
    "id": "S4-names-surfaces",
    "title": "markdown_cli/names.rs (builtins/keywords, defined functions, paths, variables, external list) and markdown_cli/surfaces.rs (hooks/dist, scripts/*.ts, bun-run .ts/.js, stable scripts; stem table; owner fallback; ported = present without placeholder verb; plugin Markdown only)",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked markdown_cli::names && cargo test -p xtask --locked markdown_cli::surfaces",
    "ac_refs": [
      "AC-4",
      "AC-5"
    ],
    "depends_on": [
      "S1-shell-lexer"
    ],
    "paths": [
      "crates/xtask/src/markdown_cli/names.rs",
      "crates/xtask/src/markdown_cli/surfaces.rs",
      "crates/xtask/src/markdown_cli/tests/names_test.rs",
      "crates/xtask/src/markdown_cli/tests/surfaces_test.rs",
      "docs/cli/commands.json"
    ],
    "input": "commands ast-grep, mod.sh, judge (defined), \"$JEV_BUN\", plugins/x/hooks/dist/y.js, export, [; real tree and a copy with ledger's planned replaced by run: bun \"$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js\" preflight, bun \"$S/launch-issue.ts\", jev.sh, bun test packages/x.test.ts (not a surface), brainstorm/delivery-flow already ported",
    "model": "inherit"
  },
  {
    "id": "S5-allowlists",
    "title": "markdown_cli/allow.rs: load and validate tooling/conventions/markdown-cli.json and tooling/conventions/markdown-cli/<plugin>.json (external, allow with file/subject/reason; unknown keys, empty reason, out-of-scope file → setup error), excuse exact matches, report stale entries",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test -p xtask --locked markdown_cli::allow",
    "ac_refs": [
      "AC-7"
    ],
    "depends_on": [
      "S2-markdown-scan"
    ],
    "paths": [
      "crates/xtask/src/markdown_cli/allow.rs",
      "crates/xtask/src/markdown_cli/tests/allow_test.rs"
    ],
    "input": "temp allowlist files: a valid entry excusing one finding, a stale entry, an entry without reason, a plugin file naming another plugin's file, an unknown key, invalid JSON, absent files",
    "model": "inherit"
  },
  {
    "id": "S6-task-and-real-repo",
    "title": "markdown_cli.rs run + TASKS/USAGE + inventory.json; tooling/conventions/markdown-cli.json with the external list and the two allowances; fix the stale mod.sh wrapper name in the ast-grep advanced reference; integration tests spawning the xtask binary on temp roots (AC-1..AC-8) and on this repository",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask check-markdown-cli && cargo test -p xtask --locked --test markdown_cli && cargo test -p xtask --locked markdown_cli && cargo xtask guardrails",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8"
    ],
    "depends_on": [
      "S2-markdown-scan",
      "S3-tree-judge",
      "S4-names-surfaces",
      "S5-allowlists"
    ],
    "paths": [
      "crates/xtask/src/main.rs",
      "crates/xtask/src/markdown_cli.rs",
      "crates/xtask/src/markdown_cli",
      "crates/xtask/src/tests/main_test.rs",
      "crates/xtask/src/tests/markdown_cli_test.rs",
      "crates/xtask/tests/markdown_cli.rs",
      "tooling/conventions/guardrails/rust/inventory.json",
      "tooling/conventions/markdown-cli.json",
      "plugins/ast-grep/skills/ast-grep/references/ast-grep-advanced.md",
      "plugins",
      "AGENTS.md",
      "docs",
      "docs/cli/commands.json",
      "tooling/conventions/markdown-cli",
      "crates/xtask/src/command_tree.rs",
      "crates/xtask/src/tests/command_tree_test.rs",
      "crates/xtask/src/cli_compat/compare.rs"
    ],
    "input": "this repository; temp roots with the real tree plus a skill with toolu epic strat on line 12, toolu --jsn epic planned, a fenced ast-grep run with the real external list and with ast-grep removed, the real delivery-flow execution.md under a ledger-ported tree, babysit planned renamed to tick then step, allowlist cases; elapsed time of the spawned binary on this repository",
    "model": "inherit"
  },
  {
    "id": "S7-ci-wiring",
    "title": "package.json check:markdown-cli in test:ts and test:docs; tests.yml installs the pinned toolchain with rust-cache in the gate and docs jobs; changes.test.ts proves a rename diff (crates + docs/cli/commands.json) turns on docs",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; bun test --timeout 60000 tooling/src/ci-paths/__tests__/changes.test.ts tooling/src/ci-paths/__tests__/workflows.test.ts && bun run check:ci-paths && bun run check:markdown-cli",
    "ac_refs": [
      "AC-6",
      "AC-9"
    ],
    "depends_on": [
      "S6-task-and-real-repo"
    ],
    "paths": [
      "package.json",
      ".github/workflows/tests.yml",
      ".github/ci-paths.json",
      "tooling/src/ci-paths",
      "tooling/src/ci-changes.ts",
      "tooling/src/check-ci-paths.ts"
    ],
    "input": "the real .github/ci-paths.json and ci-changes.ts in a git sandbox with an edit to crates/core/runtime/src/lib.rs and docs/cli/commands.json; the real tests.yml",
    "model": "inherit"
  },
  {
    "id": "S8-docs",
    "title": "docs/markdown-cli.md (scope, commands, removed surfaces and stem table, allowlist format, what a port changes); AGENTS.md CI rows, Key files xtask task list, Rust conventions Commands, Contributing; the page itself passes the gate",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask check-markdown-cli && test -f docs/markdown-cli.md && grep -q 'check-markdown-cli' AGENTS.md && grep -q 'markdown-cli.md' AGENTS.md && bun run guardrails && bun run check:opencode-surface",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "S7-ci-wiring"
    ],
    "paths": [
      "docs/markdown-cli.md",
      "AGENTS.md",
      "tooling/conventions/markdown-cli.json",
      "tooling/conventions/markdown-cli",
      "tools/toolu-opencode/generated/skills/ast-grep-ast-grep/references/ast-grep-advanced.md"
    ],
    "input": "the new page and AGENTS.md as scanned by the gate itself",
    "model": "inherit"
  },
  {
    "id": "S9-full-gates",
    "title": "Full quality gates: cargo xtask gate (Rust bar incl. coverage, jscpd, docs-cli, inventory) and bun run test:docs under the ledger's own job lease; bun run test recorded against an origin/main baseline",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(xtask): Markdown–CLI drift gate (#444)' && bun run test:docs",
    "ac_refs": [
      "AC-3",
      "AC-9"
    ],
    "depends_on": [
      "S8-docs"
    ],
    "paths": [
      "crates",
      "Cargo.toml",
      "Cargo.lock",
      "tooling",
      "package.json",
      ".github",
      "docs",
      "AGENTS.md",
      "plugins"
    ],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create: `crates/xtask/src/markdown_cli.rs`, `crates/xtask/src/markdown_cli/{scan,shell,words,judge,names,surfaces,allow}.rs`, `crates/xtask/src/markdown_cli/tests/{scan,shell,words,judge,names,surfaces,allow}_test.rs`, `tooling/conventions/markdown-cli/ast-grep.json`, `crates/xtask/src/tests/markdown_cli_test.rs`, `crates/xtask/tests/markdown_cli.rs`, `tooling/conventions/markdown-cli.json`, `docs/markdown-cli.md`.
- Modify: `crates/xtask/src/main.rs`, `tooling/conventions/guardrails/rust/inventory.json`, `package.json`, `.github/workflows/tests.yml`, `tooling/src/ci-paths/__tests__/changes.test.ts`, `AGENTS.md`, `plugins/ast-grep/skills/ast-grep/references/ast-grep-advanced.md`.

## Verification

- End to end: `cargo xtask check-markdown-cli` on this repository exits 0 in milliseconds; each issue acceptance item has an integration test spawning the real binary on a temp root built from the real tree and real Markdown (S6).
- Failure and boundary: unknown verb and flag, bad flag value, missing verb or argument only in fences, placeholder command positions, external names off the list, removed surfaces under a ported tree, stale/invalid allowlists (exit 1 vs 2), unterminated fences, quotes and heredocs.
- CI: the `docs` group fires on a rename diff (S7); `check:ci-paths` stays green with the new steps.
- Docs: `docs/markdown-cli.md` and `AGENTS.md` updated and scanned by the gate itself (S8).
- Full gates: S9's ledger check runs `cargo xtask gate` and `bun run test:docs`. `bun run test` also runs once through the job lease; its known root-host failures (comemory `be52369e`: chmod as root, merged `/bin` PATH, `bench:shell` under load) are compared against a clean `origin/main` worktree and recorded, and CI is the authority for them.
- Delivery: scoped `feat(xtask)` commits pushed after each green step; `plan-ledger.js run <plan> --verify`; `toolu-review:review` with version 2 push-review state; `verdict.js status` at `overall: ready`; PR titled `feat(xtask): Markdown–CLI drift gate (#444)` with `Closes Falconiere/toolu#444` / `Part of Falconiere/toolu#402`; then `pr-babysit:babysit`.

## Plan review

- S6: 🟡 should-fix (fixed): `paths` omitted `crates/xtask/src/tests/markdown_cli_test.rs`, the co-located test of `markdown_cli.rs`.
- S9 / Verification: 🟡 should-fix (fixed): the plan named `bun run test` but its known root-host failures would make a ledger check unrunnable; S9 checks `cargo xtask gate` and `test:docs`, and the full `bun run test` run is recorded against an `origin/main` baseline.
- Verification: 🟡 should-fix (fixed): the delivery sequence (ledger verify, review state, verdict, PR body, babysit) was missing.
- AC coverage: every spec AC (AC-1…AC-9) appears in at least one step's `ac_refs`.

## Deviations

- **Per-plugin allowlists live in `tooling/conventions/markdown-cli/<plugin>.json`, not `plugins/<plugin>/markdown-cli.json`.** `cargo xtask guardrails` rejects a new file at a plugin root: the plugin folder allowlist is `structure.plugin` in `rules.json`, gate data that changes only in its own `chore(gates):` PR. The tooling location was Jev's runner-up (0.44) and also keeps the file out of the shipped plugin. A file there must be named after a directory under `plugins/`.
- **`markdown_cli/words.rs`** holds `Command`, `Lexed`, `unquote` and the command-start rule, split from `shell.rs` to keep it under 300 code lines (rule 1).
- **The OpenCode generated surface** copies the ast-grep advanced reference, so `bun run generate:opencode-surface` regenerates it with the `mod.sh` fix (`check:opencode-surface` in S8).
- **`crates/xtask/src/command_tree.rs`** holds the tree-JSON readers (`list`, `text`, `flag_set`) that `cli_compat/compare.rs` and `markdown_cli/judge.rs` both need; jscpd (rule 16) failed S9 on the copy.
- **Review fixes:** `shell/redirect.rs` and `shell/case.rs` (child modules with their own tests) hold redirections, heredocs and `case` arms so `shell.rs` stays within the impl and file limits; array assignments, `+=`, blockquoted fences, `bun run`/`bun --cwd` scripts, valueless flags given a value, `--h`, exact stem matching and non-`NotFound` listing errors were review findings, each with a test.
- **S9 runs its gates without `job.ts`:** the ledger already holds the epic's job lease for each check, and the job capacity is 1, so a nested `job.ts` would always be refused.
- **Real findings settled in S6:** the ast-grep advanced reference's stale `mod.sh ast-grep …` became `plugins/ast-grep/hooks/dist/ast-grep.js …` (the form `docs/ast-grep/README.md` uses); the ast-grep skill's Strictness block, a list of flag values in a `bash` fence, has a per-plugin allowance; `docs/cli/installer.md` names the Node installer's own `toolu` bin twice and gets two repository allowances beside the planned two. Functions defined in one block count for the file's later blocks (the Jev problem-solving reference defines `judge()` once).
