# toolu-shell: Bash/Shell command analysis in Rust — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-toolu-shell-design.md   **Topic:** #416. Port `@toolu/core/shell` to `crates/core/shell` on tree-sitter-bash, with a shared analysis fixture, fuzzing and CI.

## Evidence and approach

The spec decides the design, and the brainstorm records the Jev calls and the measurements: cargo-deny, tree-sitter trees, nesting, latency, the parse budget and musl. Port sources, each read in full, are in `packages/toolu-core/src/shell/`:
- `shell-parse.ts`, `shell-walk.ts`, `shell-words.ts`, `shell-argv.ts`;
- `shell-options.ts`, `shell-git.ts`, `shell-writes.ts`, `shell-rules.ts`, `shell-types.ts`;
- their tests, `shell-*.test.ts`, and `parity-helpers.ts`;
- `issue-283.test.ts`.

The bats-parity case helpers live in `packages/toolu-core/src/detect/__tests__/detect-git.test.ts` and `packages/toolu-core/src/gates/__tests__/bash-commands.test.ts`.

Crate conventions follow `crates/core/protocol`:
- unit tests in `src/tests/<module>_test.rs`, wired by `#[cfg(test)] #[path = …] mod tests;`;
- black-box tests in `tests/*.rs` with `#[path = "helpers/…"]` helpers;
- the shared-fixture test pattern of `tests/encode_fixture.rs`.

The fuzz layout copies `fixtures/guardrails/rust/fuzz/clean`.

Probe crates under the scratchpad (not committed) produced the tree shapes behind the Mapping notes:
- `[ … ]` and `[[ … ]]` are both `test_command`;
- `(( … ))` is a `compound_statement`;
- heredoc continuations sit inside `heredoc_redirect`;
- a backslash-newline splits a word;
- `select` is a `for_statement`.

`cargo` on this host must be the rustup proxy (`$HOME/.cargo/bin`), so every check sets `PATH`. Full gates and long test runs go through the epic job wrapper (`job.ts`).

## Workstream summary

Dependencies and records → option parser → parse and words → wrappers → walk → git and rules → writes → TypeScript projection and the captured fixture → Rust shared-fixture and case tests → limits, malformed input, scenarios and latency → fuzz package and local runs → CI (fuzz job, latency step, musl compiler, scheduled workflow) → documentation → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "deps-records",
    "title": "tree-sitter 0.24 and tree-sitter-bash 0.23 in [workspace.dependencies] and toolu-shell; analysis.rs records (ShellAnalysis, ShellCommand, ShellRedirect, Heredoc, RedirectOperator, CommandOrigin, PipelinePosition, ShellError, Tristate) with as_str tests; options.rs getopt port with shell-options cases",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-shell --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t analysis::tests:: && t options::tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo deny --all-features check bans licenses sources",
    "ac_refs": [
      "AC-8",
      "AC-9"
    ],
    "paths": [
      "crates/core/shell/",
      "Cargo.toml",
      "Cargo.lock",
      "deny.toml"
    ],
    "input": "parseArgs cases from shell-git/argv/writes tests: -am clusters, -i.bak rest letters, --name=value, numeric -10, + clusters, a trailing value option (missingValue), -- and - operands",
    "model": "inherit"
  },
  {
    "id": "parse-words",
    "title": "parse.rs (tree-sitter parser, PARSE_BUDGET progress callback, ERROR/MISSING errors, MAX_SHELL_INPUT in UTF-16 units) and words.rs/words/quote.rs/heredoc.rs (static values, quote removal, ANSI-C, globs with merged literal parts, heredoc content and heredoc-cat) with unit tests",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-shell --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t parse::tests:: && t words::tests:: && t words::quote::tests:: && t heredoc::tests::",
    "ac_refs": [
      "AC-7",
      "AC-9"
    ],
    "depends_on": [
      "deps-records"
    ],
    "paths": [
      "crates/core/shell/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "words from shell-walk.test.ts ('a b' \"c $d\" $'e\\tf' a\\ b .en[v] *.log \"$(cat <<'EOF'…EOF)\"); boundary: 1 MiB+1 UTF-16 units, an astral-plane character counted as 2, 1 MiB of ${ cancelled by the budget, a <<- body with tabs, an unquoted body with $",
    "model": "inherit"
  },
  {
    "id": "argv",
    "title": "argv.rs: WRAPPERS option tables, unwrap, align_unwrapped, run_target (bash/sh/zsh/dash/ksh -c/+c/-s/stdin, eval) ported with every shell-argv.test.ts case",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-shell --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t argv::tests::",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "deps-records"
    ],
    "paths": [
      "crates/core/shell/"
    ],
    "input": "shell-argv.test.ts: sudo -u me timeout 60 git push, env -S, command -v, xargs, nice -10, /usr/bin/sudo, bash +o posix -c, eval -- x; boundary: sudo -l, env -, a dynamic wrapper operand",
    "model": "inherit"
  },
  {
    "id": "walk",
    "title": "walk.rs (+ walk/ child modules) and command.rs: statements, flattened and-or lists, pipelines, compound bodies, [ ] and [[ ]], (( )), coproc, functions, case/for/select/while/if, heredoc continuations, line-continuation joins, nested scripts with MAX_NESTING, bash -c/eval re-parse to MAX_RUN_DEPTH, analyze() entry; shell-walk.test.ts and shell-parse.test.ts cases plus the probed constructs",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-shell --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t walk:: && t command::tests:: && t tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only guardrails",
    "ac_refs": [
      "AC-3",
      "AC-4",
      "AC-7",
      "AC-9"
    ],
    "depends_on": [
      "parse-words",
      "argv"
    ],
    "paths": [
      "crates/core/shell/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "shell-walk.test.ts and shell-parse.test.ts inputs; probed: node -\\<nl>e x, [ -f \"$x\" ] || [ a = b ], coproc git push, cat <<EOF | bash, cat <<'EOF' >f && git push, { echo x; } 2>&1 >log &, f() { :; } > out; boundary: ), if, git push; echo \"unterminated, 10,000 nested $(…)",
    "model": "inherit"
  },
  {
    "id": "git-rules",
    "title": "git.rs (git_invocation, runs_git_subcommand, push_targets with Refspec and destination, commit_messages) and rules.rs (matches_rule) with every shell-git.test.ts and shell-rules.test.ts case",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-shell --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t git::tests:: && t rules::tests::",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "walk"
    ],
    "paths": [
      "crates/core/shell/"
    ],
    "input": "git -C a -C b push origin HEAD:x, +HEAD:refs/heads/x, :x, HEAD, wildcard refspec, $g push, git $(echo push), git commit -am msg -m \"$(cat <<'EOF'…)\"; rules sudo node -e, cd /tmp && node -e, an empty rule",
    "model": "inherit"
  },
  {
    "id": "writes",
    "title": "writes.rs (+ writes/copy.rs, writes/python.rs): redirect, tee, sed -i/perl -i (BSD ''), cp/mv/install with -t and DEST/basename(SRC), dd of=, python -c/stdin open() literal parsing without backreference regexes, compound redirects; every shell-writes.test.ts case",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-shell --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t writes::",
    "ac_refs": [
      "AC-7",
      "AC-9"
    ],
    "depends_on": [
      "walk"
    ],
    "paths": [
      "crates/core/shell/"
    ],
    "input": "shell-writes.test.ts: echo x >.env, 2>&1, >&-, cp a b c dir/, install -d, -t DIR, sed -i '' s/a/b/ f, python3 -c \"open('x','w')\", Path(…).open, f-string path, heredoc python; boundary: dynamic targets ($HOME/.env), glob targets (.e?v)",
    "model": "inherit"
  },
  {
    "id": "ts-projection",
    "title": "projectAnalysis in parity-helpers.ts, analysis-fixture.test.ts (inputs equal unbash-baseline.json in order; TS reproduces every expect), fixtures/shell/analysis.json captured once from TypeScript for the 203 baseline inputs",
    "check": "bun test packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts && bun run tooling/src/check-unbash-baseline.ts && bun x oxfmt --check packages/toolu-core/src/shell/__tests__/ && bun run typecheck",
    "ac_refs": [
      "AC-2"
    ],
    "paths": [
      "fixtures/shell/",
      "packages/toolu-core/src/shell/",
      "tooling/src/check-unbash-baseline.ts",
      "bun.lock"
    ],
    "input": "fixtures/shell/unbash-baseline.json (203 inputs) through analyzeShell, writeTargets, gitInvocation, pushTargets, commitMessages, runsGitSubcommand",
    "model": "inherit"
  },
  {
    "id": "rust-fixture",
    "title": "tests/analysis_fixture.rs with helpers/project.rs: Rust projection equals expect (or rust.expect with rust.reason) for all 203 cases; intended differences recorded in the fixture and listed in docs/shell-analysis.md (analysis-fixture.test.ts asserts every rust.reason appears there)",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-shell --test analysis_fixture 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && bun test packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts",
    "ac_refs": [
      "AC-2"
    ],
    "depends_on": [
      "git-rules",
      "writes",
      "ts-projection"
    ],
    "paths": [
      "crates/core/shell/",
      "fixtures/shell/",
      "packages/toolu-core/src/shell/",
      "docs/shell-analysis.md",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "fixtures/shell/analysis.json; boundary: a rust override whose reason is missing from the docs fails the TS test",
    "model": "inherit"
  },
  {
    "id": "fixture-cases",
    "title": "tests/fixture_cases.rs: every bats-parity.json and issue-283.json case through the crate API (git subcommand, write targets, decide rule over matches_rule/text, push root via real git -C replay, push branch attached/detached, commit messages, commands, exit proves, latency)",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-shell --test fixture_cases 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-1"
    ],
    "depends_on": [
      "git-rules",
      "writes"
    ],
    "paths": [
      "crates/core/shell/",
      "fixtures/shell/bats-parity.json",
      "fixtures/shell/issue-283.json"
    ],
    "input": "186 bats-parity cases and 49 issue-283 cases; real git repositories created with git init under a tempfile dir for push_target_root/branch ($TMP/$MKTEMP layouts, detached HEAD)",
    "model": "inherit"
  },
  {
    "id": "limits-scenarios",
    "title": "tests/limits.rs (10,000 nested $(…) node -e on a 2 MiB thread → unknown + nesting error + decide unknown; 100,000-term arithmetic and && chains), tests/malformed.rs (AC-3 lines), tests/scenarios.rs (issue scenarios, oversize message, ${ budget)",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-shell --test \"$1\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t limits && t malformed && t scenarios",
    "ac_refs": [
      "AC-3",
      "AC-4",
      "AC-7"
    ],
    "depends_on": [
      "git-rules",
      "writes"
    ],
    "paths": [
      "crates/core/shell/"
    ],
    "input": "generated: \"$(\"×10,000 + \"node -e x\" + \")\"×10,000; $((1+…)) ×100,000; true&&…×100,000; git push origin main; echo \"unterminated; git push origin main\\necho 'x; ); if; git status && bash -c \"rm -rf x\" | head; echo x >.env; 'a'×1,048,577; '${'×524,288",
    "model": "inherit"
  },
  {
    "id": "latency",
    "title": "tests/latency.rs: p99 of analyze + runs_git_subcommand + push_targets + write_targets over the 235 real commands of bats-parity.json and issue-283.json (bench:shell's fixtureCommands) × 200 rounds; ≤100 µs in release, 10 ms smoke ceiling otherwise",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test --release -p toolu-shell --test latency -- --nocapture 2>&1 | grep -Eq 'test result: ok\\. 1 passed'",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "rust-fixture"
    ],
    "paths": [
      "crates/core/shell/",
      "fixtures/shell/bats-parity.json",
      "fixtures/shell/issue-283.json",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "the 235 commands of fixtures/shell/bats-parity.json and issue-283.json",
    "model": "inherit"
  },
  {
    "id": "fuzz",
    "title": "crates/core/shell/fuzz: Cargo.toml ([workspace], libfuzzer-sys, toolu-shell path dep), nightly rust-toolchain.toml, .gitignore (target corpus artifacts Cargo.lock), fuzz_targets/analyze.rs and fuzz_targets/nested.rs; ≥10 min local run per target with a fixture-seeded corpus and no crash",
    "check": "python3 -c 'import json,os,sys; d=sys.argv[1]; os.makedirs(d, exist_ok=True); [open(os.path.join(d, \"seed-%03d\" % i), \"w\").write(c[\"input\"]) for i, c in enumerate(json.load(open(\"fixtures/shell/unbash-baseline.json\"))[\"cases\"])]' crates/core/shell/fuzz/corpus/analyze && python3 -c 'import json,os,sys; d=sys.argv[1]; os.makedirs(d, exist_ok=True); [open(os.path.join(d, \"seed-%03d\" % i), \"w\").write(c[\"input\"]) for i, c in enumerate(json.load(open(\"fixtures/shell/unbash-baseline.json\"))[\"cases\"])]' crates/core/shell/fuzz/corpus/nested && cd crates/core/shell/fuzz && PATH=\"$HOME/.cargo/bin:$PATH\" cargo +nightly fuzz build && PATH=\"$HOME/.cargo/bin:$PATH\" cargo +nightly fuzz run analyze -- -max_total_time=60 -timeout=10 && PATH=\"$HOME/.cargo/bin:$PATH\" cargo +nightly fuzz run nested -- -max_total_time=60 -timeout=10 && cd ../../../.. && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only guardrails",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "git-rules",
      "writes"
    ],
    "paths": [
      "crates/core/shell/",
      "fixtures/shell/unbash-baseline.json"
    ],
    "input": "seed corpus from fixtures/shell/unbash-baseline.json inputs plus libFuzzer mutations; the nested target's fragment grammar",
    "model": "inherit"
  },
  {
    "id": "fuzz-long",
    "title": "Local long fuzz run: each target for 600 s on the fixture-seeded corpus with a 10 s per-input timeout; no crash, panic, leak report or timeout (AC-5 evidence; the scheduled CI run is post-merge)",
    "check": "python3 -c 'import json,os,sys; d=sys.argv[1]; os.makedirs(d, exist_ok=True); [open(os.path.join(d, \"seed-%03d\" % i), \"w\").write(c[\"input\"]) for i, c in enumerate(json.load(open(\"fixtures/shell/unbash-baseline.json\"))[\"cases\"])]' crates/core/shell/fuzz/corpus/analyze && python3 -c 'import json,os,sys; d=sys.argv[1]; os.makedirs(d, exist_ok=True); [open(os.path.join(d, \"seed-%03d\" % i), \"w\").write(c[\"input\"]) for i, c in enumerate(json.load(open(\"fixtures/shell/unbash-baseline.json\"))[\"cases\"])]' crates/core/shell/fuzz/corpus/nested && cd crates/core/shell/fuzz && PATH=\"$HOME/.cargo/bin:$PATH\" cargo +nightly fuzz run analyze -- -max_total_time=600 -timeout=10 && PATH=\"$HOME/.cargo/bin:$PATH\" cargo +nightly fuzz run nested -- -max_total_time=600 -timeout=10",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "fuzz"
    ],
    "paths": [
      "crates/core/shell/src/",
      "crates/core/shell/fuzz/",
      "crates/core/shell/Cargo.toml",
      "fixtures/shell/unbash-baseline.json",
      "Cargo.lock"
    ],
    "input": "203 fixture inputs as seeds plus libFuzzer mutations for 10 minutes per target",
    "model": "inherit"
  },
  {
    "id": "ci",
    "title": "tests.yml: fuzz job (group rust, nightly + cargo-fuzz, lockfile copy, seeded corpus, 60 s per target) in the typescript aggregate needs; rust job runs the release latency test; rust-musl installs musl-tools and sets CC_<target>=musl-gcc; .github/workflows/fuzz.yml daily + workflow_dispatch, 30 min per target, artifact upload on failure",
    "check": "bun run check:ci-paths && bun test tooling/src/ci-paths/__tests__/ && CC_x86_64_unknown_linux_musl=musl-gcc PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-shell --target x86_64-unknown-linux-musl --test scenarios 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && grep -Eq 'cron:' .github/workflows/fuzz.yml && grep -q 'fuzz run analyze' .github/workflows/fuzz.yml && grep -q 'fuzz run nested' .github/workflows/fuzz.yml && grep -q 'fuzz run analyze' .github/workflows/tests.yml && grep -q 'fuzz run nested' .github/workflows/tests.yml",
    "ac_refs": [
      "AC-5",
      "AC-6",
      "AC-8"
    ],
    "depends_on": [
      "fuzz",
      "latency",
      "limits-scenarios"
    ],
    "paths": [
      ".github/",
      "tooling/src/",
      "crates/core/shell/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "the workflow files and .github/ci-paths.json; a static x86_64 musl test binary linking tree-sitter, built with musl-gcc as the rust-musl job will",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/shell-analysis.md Rust section (crate API, parser decision, limits, fuzzing, intended differences), docs/rust-quality-bar.md fuzzer decision, fixtures/shell/README.md and fixtures/README.md analysis.json, AGENTS.md key file row and CI table rows",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" bun run test:docs && bun test packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts",
    "ac_refs": [
      "AC-2",
      "AC-5",
      "AC-8"
    ],
    "depends_on": [
      "ci",
      "rust-fixture"
    ],
    "paths": [
      "docs/",
      "fixtures/README.md",
      "fixtures/shell/",
      "packages/toolu-core/src/shell/",
      "AGENTS.md",
      "CLAUDE.md"
    ],
    "input": "the changed surfaces of this PR",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gate: cargo xtask gate (coverage ≥90% toolu-shell, deny, guardrails, unused-pub, jscpd), the TypeScript conventions and the TS suites this PR touches; the full bun run test is run separately and compared with an origin/main baseline because five root-only and PATH tests fail on this host (comemory be52369e), and CI runs it in full",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main --title 'feat(shell): Bash/Shell command analysis in Rust (#416)' && bun run test:conventions && bun test --timeout 60000 packages/toolu-core/src/shell tooling/src/__tests__/check-unbash-baseline.test.ts tooling/src/ci-paths && bun run check:ci-paths && PATH=\"$HOME/.cargo/bin:$PATH\" bun run test:docs",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "docs",
      "fixture-cases",
      "limits-scenarios"
    ],
    "paths": [
      "crates/",
      "Cargo.toml",
      "Cargo.lock",
      "fixtures/",
      "packages/",
      "docs/",
      "tooling/",
      ".github/",
      "AGENTS.md"
    ],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create:
  - `crates/core/shell/src/{analysis,options,parse,words,heredoc,argv,walk,command,git,rules,writes}.rs` and their child modules (`words/quote.rs`, `walk/*.rs`, `writes/{copy,python}.rs`);
  - `crates/core/shell/src/tests/*_test.rs` and the child modules' `tests/`;
  - `crates/core/shell/tests/{analysis_fixture,fixture_cases,limits,malformed,scenarios,latency}.rs` and `tests/helpers/*.rs`;
  - `crates/core/shell/fuzz/{Cargo.toml,rust-toolchain.toml,.gitignore,fuzz_targets/analyze.rs,fuzz_targets/nested.rs}`;
  - `fixtures/shell/analysis.json`, `packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts` and `.github/workflows/fuzz.yml`.
- Modify:
  - `Cargo.toml`, `Cargo.lock`, `crates/core/shell/{Cargo.toml,src/lib.rs}`;
  - `packages/toolu-core/src/shell/__tests__/parity-helpers.ts` and `.github/workflows/tests.yml`;
  - `docs/shell-analysis.md`, `docs/rust-quality-bar.md`, `fixtures/README.md`, `fixtures/shell/README.md` and `AGENTS.md`.

## Verification

- **Real inputs:**
  - the 203 baseline inputs, through both implementations (`analysis.json`);
  - the 235 bats-parity and #283 cases, through the crate API with real git repositories;
  - generated deep inputs on a 2 MiB thread;
  - pathological 1 MiB inputs under the budget;
  - fuzzing seeded from the fixtures.
- **Failure and boundary checks:**
  - `)` and `if` are unknown;
  - a push before an unterminated quote is still reported;
  - oversize input gets the TypeScript message;
  - a nesting overflow is unknown;
  - a cancelled parse is unknown;
  - dynamic and glob write targets keep their `text` and `pattern`.
- **Documentation:** `test:docs` covers the changed docs and the AGENTS.md sync surface. Intended differences are listed in `docs/shell-analysis.md` and carried in `analysis.json` with their reasons.
- **Final:** `plan-ledger.js run <plan> --verify`, `toolu-review:review`, then `verdict.js status`.

## Plan review

**Status:** Approved (2026-10-06, epic worker). Mechanical checks: all 9 spec ACs are mapped, with no dangling `ac_refs`, every `depends_on` points to an earlier step, the ids are unique, and every check is non-empty. Jev scored step-to-AC alignment, first pass then revised: AC-1 0.81; AC-2 0.62 → 0.85; AC-8 0.50 → 0.79; AC-5 0.71 → 0.63.

- `gate`: 🟡 should-fix (fixed). `bun run test` fails on this root host for environmental reasons that are also on `origin/main` (comemory be52369e). The local check now runs `test:conventions`, the touched suites and `test:docs`. The full run is compared against a baseline and is required green in CI.
- `ci`: 🟡 should-fix (fixed). Musl was unverifiable locally. The check now builds and runs a static x86_64 musl test binary with `musl-gcc`, and checks that `fuzz.yml` and `tests.yml` run both targets on a schedule and per PR. It also gained the missing `limits-scenarios` dependency.
- `rust-fixture`/`docs`: 🟡 should-fix (fixed). Documented intended differences were not checked. `analysis-fixture.test.ts` now asserts that every `rust.reason` appears in `docs/shell-analysis.md`.
- `fuzz-long`: 🟡 should-fix (fixed). The 10-minute local runs are now a ledger step with a check.
- AC-5: 🔵 accepted limitation. GitHub runs scheduled workflows only from the default branch, so the scheduled run is post-merge evidence. Jev's residual 0.63 reflects exactly that, and the PR states it.

## Deviations

- `deps-records` (2026-10-06): `tree-sitter` 0.27 with `tree-sitter-bash` 0.25 failed `cargo deny` licences inside the workspace, though they passed in an isolated probe. The cause is `foldhash` (Zlib). It reaches a non-dev path through `tree-sitter`'s non-optional `serde_json/preserve_order` build dependency, then `indexmap`, then `hashbrown`, whose default features the `jsonschema` dev-dependency turns on. Every `tree-sitter` from 0.25 to 0.27 has that build dependency, and 0.24.7 does not. The pair is now `tree-sitter` 0.24.7 with `tree-sitter-bash` 0.23.3 (ABI 14), which gives `bans ok, licenses ok, sources ok`. A re-probe gave identical results on the 203 fixture inputs. Only `(( … ))` maps differently, and the spec covers it. The budget uses `set_timeout_micros`. The spec is updated, and no gate data changes.
- `latency` (2026-10-06): the timed set is now the 235 real commands that `bench:shell`'s `fixtureCommands` reads, not the 203 baseline inputs. The baseline includes the two synthetic malformed inputs, and tree-sitter's error recovery takes about 100 µs on `echo $(unterminated`, which alone set the old p99 at 100–102 µs. AC-6 says "real-command fixture set", and the spec is updated to name it.
- `ts-projection` (2026-10-06): the projection lives in `__tests__/analysis-projection.ts`, not in `parity-helpers.ts`, which other suites import.
- `docs`, `gate` (2026-10-06): `bun run test:docs` now runs `cargo xtask check-markdown-cli`, which needs the pinned toolchain. Both checks put `$HOME/.cargo/bin` first on `PATH`, as the Rust checks already do, because the host's system `cargo` is 1.93.
- `ts-projection` (2026-10-06): the check typechecks instead of running `bun x oxlint` on `__tests__`. Core tests are a declared oxlint gap in `tooling/gate-reach.json`, and the house plugin cannot load from the repository root, so that segment could never pass and would lint nothing.
- `fuzz-long` (2026-10-06): the 600 s run of `analyze` found a second scanner-state abort. The old estimate ended a delimiter at `|&;<>()` and skipped `<` runs three at a time. tree-sitter-bash reads a delimiter up to whitespace and stores a NUL after it. The bound now lives in `src/scanner.rs`:
  - one 7-byte push per run of `<` not followed by `=`;
  - each delimiter the scanner can read after a `<<` token left by a lexer, with its NUL.

  Here-string text is not counted, so the real `gh api … <<< '{…}'` fixture command stays known (Jev: 0.86 for this bound over counting every overlapping `<<`). `tests/limits.rs` now pins the overflow window (1,013 to 1,015 characters aborted before the change). The crash input, the window and 16 here-string probes replay clean.
- `gate` (2026-10-06): `crates/xtask/tests/no_exemptions.rs` walked untracked build output under `crates/`, which a local `cargo fuzz` build puts in `crates/core/shell/fuzz/target`. It now skips directories cargo tags with `CACHEDIR.TAG`, and a test pins that.
