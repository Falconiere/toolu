# toolu-state: gate file, telemetry, edit records and git facts without spawning git — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-toolu-state-design.md   **Topic:** #415. Port `@toolu/core/state` and the shell-free half of `@toolu/core/detect` to `crates/core/state`; read the per-call git facts from `.git` (discovery in `toolu-runtime`).

## Evidence and approach

The spec decides the design (D1–D6); the brainstorm records the Jev calls.

**Port sources:**
- `packages/toolu-core/src/state/{state-io,state-schema,gate-file,telemetry,edit-records,state-sweeper,diff-sha,state-git}.ts`;
- `packages/toolu-core/src/detect/{detect-branch,detect-project,detect-tools,detect-lines,detect-read}.ts`;
- `packages/toolu-core/src/quality/quality-edit.ts` (`inLinkedWorktree` semantics).

**Unit-case sources:** `packages/toolu-core/src/state/__tests__` and `packages/toolu-core/src/detect/__tests__`, plus `fixtures/state/cases.json` (77 cases; `package` is TypeScript-only).

**Reused from `toolu-runtime`:**
- `json::{jq_text, js_number, ordered::Ordered}`;
- `host::roots::Roots::{project_root, project_state_root, project_state_dir}`;
- `config::{load::load, read::enabled}`;
- `process::{run, Spec}` for every spawn;
- `env::Env`.

**Integration-test conventions:**
- `crates/core/runtime/tests/helpers/repo.rs` (`#[path]` helper, `Res<T>`);
- the `harness = false` self-exec precedent `crates/core/protocol/tests/run_hook.rs`.

**Feasibility probes (scratch, Bun 1.4):**
- zod emits extras in schema order: `push_check` → `result, reason_code, round`.
- `JSON.parse` orders canonical array-index keys below 2³²−1 first: `{"b","10","4294967295","4294967294"}` → `10, 4294967294, b, 4294967295`.
- `bun install --frozen-lockfile` completes in this worktree.
- `cargo` must be the rustup proxy (`$HOME/.cargo/bin`); `/usr/bin/cargo` is a system cargo. Every check sets `PATH`.

**Constraints:**
- Rule 14 (`std::process::Command` only in `toolu-runtime::process`; no stderr from `src`). Capability rules scan `src` only.
- Comemory `be52369e`: root ignores `chmod`, so no test relies on an unwritable directory; a regular file is placed where a directory must be.
- `docs/toolu` is gitignored, and design docs are force-added as in #413/#414.

**Goldens:** `fixtures/state/gate-bytes.json` and `fixtures/state/edit-records.json` are captured once by a scratch script outside the repository that imports the TypeScript modules (#413/#414 practice). A Bun test and a Rust test then assert each.

## Workstream summary

Runtime discovery and raw stdout → state foundation (ctx, time, atomic write, lock) → git facts → telemetry → gate file → gate-bytes golden → edit records and their golden → diffSha → the shared state cases → sweeper → detect → the TS/Rust interleave test → dependency check → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "runtime-discovery",
    "title": "toolu-runtime: git module (Discovery, Repo, discover, toplevel) mirroring setup_git_directory_gently_1 with AskGit deferrals (GIT_* vars, foreign owner via nix user feature, core.worktree, core.bare, config.worktree), filesystem-boundary stop; Roots::project_root and config::quality use git::toplevel; process::Output gains stdout_bytes",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t git::tests:: && t git::gitdir::tests:: && t git::defer::tests:: && t process::tests:: && t host::roots::tests:: && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-runtime --test host_roots_fixture --test config_fixture 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-5",
      "AC-6",
      "AC-11"
    ],
    "paths": [
      "crates/core/runtime/",
      "Cargo.lock",
      "fixtures/host/root.json",
      "fixtures/config/"
    ],
    "input": "real repos from git init / worktree add / init --separate-git-dir / init --bare in temp dirs; cwd through a symlink; a .git file with garbage; a .git dir missing objects/; GIT_DIR set in the Env; config with core.worktree and bare = true; a child printing bytes 0xff 0xfe",
    "model": "inherit"
  },
  {
    "id": "foundation",
    "title": "toolu-state crate deps (toolu-runtime, serde_json, nix signal, tempfile), StateCtx, iso_seconds, write_atomic (0600 tempfile beside, fsync, persist, fsync dir), with_lock (TS protocol: pid+uuid content, dead-pid and 2 s stale break via rename/link-back, 5 s unlocked fallback with warning, token-checked release on Drop)",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t ctx::tests:: && t time::tests:: && t io::tests:: && t lock::tests::",
    "ac_refs": [
      "AC-2",
      "AC-4"
    ],
    "depends_on": [
      "runtime-discovery"
    ],
    "paths": [
      "crates/core/state/",
      "Cargo.lock"
    ],
    "input": "SystemTime at 2026-09-28T12:34:56.789Z, 1970 epoch, a leap day; a missing directory; a lock with a dead pid (99999), a live pid aged 3 s via set_modified, a live fresh pid with a 100 ms timeout, another holder's token, a panicking closure",
    "model": "inherit"
  },
  {
    "id": "git-facts",
    "title": "toolu_state::git: current_branch (HEAD, loose and packed refs, unborn, detached, ambiguity and reftable deferral), linked_worktree, common_dir, base_branch (origin/HEAD file), branch_slug, branch_slugs and has_git (spawn), toplevel re-export; integration test against real git incl. PATH emptied",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t git::tests:: && t git::refs::tests:: && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test git_facts 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "foundation"
    ],
    "paths": [
      "crates/core/state/",
      "crates/core/runtime/src/git.rs",
      "crates/core/runtime/src/git/"
    ],
    "input": "repos built by the test: normal (root, symlinked subdir), linked worktree, submodule (protocol.file.allow=always), detached, unborn, pack-refs --all, --separate-git-dir, .git file to a bare repo, bare, inside .git, outside any repo, branch v1 plus tag v1; Env with PATH=\"\"",
    "model": "inherit"
  },
  {
    "id": "telemetry",
    "title": "toolu_state::telemetry: closed TelemetryEvent enum in TELEMETRY_EXTRAS key order, line assembly with js numbers, 3,900-byte cap, config telemetry.enabled, branch skip, TELEMETRY_DIR override, append; strict parse_telemetry_line / parse_telemetry_extras",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t telemetry::tests:: && t telemetry_schema::tests::",
    "ac_refs": [
      "AC-9",
      "AC-4"
    ],
    "depends_on": [
      "git-facts"
    ],
    "paths": [
      "crates/core/state/src/",
      "fixtures/state/cases.json"
    ],
    "input": "git sandbox on feat/x with toolu.config.json; the eight telemetry-events fixture lines; detached HEAD; telemetry.enabled false; a 3,901-byte decision; TELEMETRY_DIR in the Env; duration_s 1.5 and exit_code 0",
    "model": "inherit"
  },
  {
    "id": "gate-file",
    "title": "toolu_state gate schema (hand validator, zod-style path reason), JS key order for entries, gate_doc (seed, sort by (updatedAt,key) bytes, failing/cleared docs, dropped count), gate_file read/record/clear with lock, single-slot fallback, drop log, unrecognized handling, gate_fail/gate_clear telemetry",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t gate_schema::tests:: && t js_order::tests:: && t gate_doc::tests:: && t gate_file::tests::",
    "ac_refs": [
      "AC-3",
      "AC-4",
      "AC-9"
    ],
    "depends_on": [
      "telemetry"
    ],
    "paths": [
      "crates/core/state/src/"
    ],
    "input": "documents: passing, legacy single-slot, multi-slot, version 1 and 1.0, version 2, unknown keys at root/passing/entry, entries not an object, null/false/empty/garbage; keys b and 10; a gate path whose directory is a regular file (atomic write fails → single-slot fallback)",
    "model": "inherit"
  },
  {
    "id": "gate-bytes-golden",
    "title": "Capture fixtures/state/gate-bytes.json from TypeScript (scratch script outside the repo), assert it in a Bun test and a Rust integration test; register the suite in fixtures/index.json and the inventory count",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test gate_bytes 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && bun test packages/toolu-core/src/state/__tests__/gate-bytes.test.ts tooling/src/__tests__/check-fixture-inventory.test.ts && bun run tooling/src/check-fixture-inventory.ts",
    "ac_refs": [
      "AC-3"
    ],
    "depends_on": [
      "gate-file"
    ],
    "paths": [
      "fixtures/state/gate-bytes.json",
      "fixtures/index.json",
      "crates/core/state/",
      "packages/toolu-core/src/state/__tests__/gate-bytes.test.ts",
      "packages/toolu-core/src/state/",
      "tooling/src/__tests__/check-fixture-inventory.test.ts",
      "tooling/src/check-fixture-inventory.ts"
    ],
    "input": "operation sequences: fresh record, second slot, re-record in place, clear with promotion, clear to passing, legacy single-slot seed, version 1 seed, integer-like key, DEL/control/non-ASCII violations, equal timestamps ordered by key bytes, non-owner clear, unrecognized replacement with drop log",
    "model": "inherit"
  },
  {
    "id": "edit-records",
    "title": "toolu_state::edit_records and apply_patch (TS port), golden fixtures/state/edit-records.json captured from TypeScript, Bun test and Rust integration test, index registration; plus parse_edit_record for the shared EditRecordSchema cases",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t edit_records::tests:: && t apply_patch::tests:: && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test edit_records 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && bun test packages/toolu-core/src/state/__tests__/edit-records.test.ts tooling/src/__tests__/check-fixture-inventory.test.ts && bun run tooling/src/check-fixture-inventory.ts",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "foundation",
      "gate-bytes-golden"
    ],
    "paths": [
      "crates/core/state/",
      "fixtures/state/edit-records.json",
      "fixtures/index.json",
      "packages/toolu-core/src/state/",
      "tooling/src/__tests__/check-fixture-inventory.test.ts",
      "tooling/src/check-fixture-inventory.ts"
    ],
    "input": "Edit/Write/MultiEdit payloads with file_path, path, target_file, null and false precedence, trailing newlines, tab/CR in path; apply_patch add/update/delete/move/EOF marker/CRLF; malformed: no Begin, double Begin, text after End, unknown *** header, Move without Update, header without space, empty patch; non-object payload and tool_input; Read tool",
    "model": "sonnet"
  },
  {
    "id": "diff-sha",
    "title": "toolu_state::diff_sha: dash ref refused without spawn, git diff raw bytes (no budget) piped to git hash-object --stdin in the repo",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t diff_sha::tests::",
    "ac_refs": [
      "AC-11"
    ],
    "depends_on": [
      "foundation"
    ],
    "paths": [
      "crates/core/state/src/",
      "crates/core/runtime/src/process.rs",
      "crates/core/runtime/src/process/"
    ],
    "input": "a feature branch adding a file holding bytes 0xff 0xfe and a 1.5 MiB file; empty diff; bad base; non-repo dir; base ref --output=planted",
    "model": "inherit"
  },
  {
    "id": "state-cases",
    "title": "Rust consumer of fixtures/state/cases.json: schema, telemetry-events, telemetry-extras, gate-file, io (jq text vs real jq, jq sort, iso, atomic, lock scenarios), branch-slug, base-branch, current-branch, branch-slugs, diff-sha; the package case is TypeScript-only and asserted absent from the run list by name; split across tests/state_cases.rs, tests/io_cases.rs and tests/gate_cases.rs (300-line file limit)",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test state_cases --test diff_cases --test io_cases --test gate_cases 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-4",
      "AC-11"
    ],
    "depends_on": [
      "gate-file",
      "diff-sha"
    ],
    "paths": [
      "crates/core/state/",
      "crates/core/runtime/src/",
      "fixtures/state/cases.json"
    ],
    "input": "fixtures/state/cases.json: 75 non-package, non-concurrency cases, including the rejecting schema cases (unknown keys, version 2, nested payload, smuggled protocol keys), malformed/null/false/empty/unrecognized gate documents, dead/stale/live/taken-over locks, a missing directory for writeAtomic, a bad and a dash-prefixed base ref, a non-repository dir, detached/unborn HEAD; the 2 concurrency cases run in interleave",
    "model": "inherit"
  },
  {
    "id": "sweeper",
    "title": "toolu_state::sweeper: branch-state TTL/merged/gone, gate-file reclaim under lock, telemetry retention trim (jq ordering), gates.sweep switch, has_git; integration test on a real repo",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t sweeper::tests:: && t sweep_telemetry::tests:: && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test sweeper 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "gate-file",
      "git-facts"
    ],
    "paths": [
      "crates/core/state/"
    ],
    "input": "repo with main, feat/merged (merged), feat/ahead (live), a deleted branch's files, a 2-day-old live file (set_modified), waiver and pending-waiver names; gate files passing / failing on a missing file / failing on a live file / __global__ / unrecognized; telemetry with old, new, null, non-string t and a bad line; config gates.sweep false; stateTtlHours 1",
    "model": "inherit"
  },
  {
    "id": "detect",
    "title": "toolu_state::detect: project probes (toplevel, name, node pm, rust, python, ts via git ls-files, ts/python linter, clippy, relative path), tool_available (PATH scan, cache), detect_ast_grep, chunked latin1 line counters; tests porting the TS layouts, bats answers, tool cases and snippets, plus a Rust-vs-TS count over every tracked source via bun",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --lib -- \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t detect::tests:: && t detect::project::tests:: && t detect::tools::tests:: && t detect::lines::tests:: && t detect::read::tests:: && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test detect 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-12"
    ],
    "depends_on": [
      "foundation",
      "runtime-discovery"
    ],
    "paths": [
      "crates/core/state/",
      "packages/toolu-core/src/detect/"
    ],
    "input": "the 35 layouts of detect-project.test.ts from root and sub/dir; isolated PATH dirs with exe, plain 0644 file, directory, dangling link, empty entry; the 16 snippets, NUL bytes, a missing file, a directory, a 1.5 MiB file; every tracked *.ts *.rs *.py *.sh file",
    "model": "sonnet"
  },
  {
    "id": "interleave",
    "title": "tests/interleave.rs (harness = false): 8 bun gate-writer.ts + 8 self-exec Rust writers race with a polling reader; crashed-holder locks across implementations; the two concurrency cases of cases.json with Rust writers",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test interleave 2>&1) && printf '%s' \"$o\" | grep -q 'interleave: ok'",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-4"
    ],
    "depends_on": [
      "gate-file"
    ],
    "paths": [
      "crates/core/state/",
      "packages/toolu-core/src/state/",
      "fixtures/state/cases.json"
    ],
    "input": "git sandbox on feat/race with config; writers record /w/<id>/0..3 and clear evens; a lock left by a Rust child that exits inside with_lock; a lock left by a bun -e child that exits inside withLock; fixture concurrency cases (16 writers, stale lock 4242 aged 60 s)",
    "model": "inherit"
  },
  {
    "id": "deps",
    "title": "tests/deps.rs: cargo tree -p toolu-state -e normal lists no crate rules.json reserves for toolu-shell or toolu-http",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-state --test deps 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-7"
    ],
    "depends_on": [
      "foundation"
    ],
    "paths": [
      "crates/core/state/",
      "Cargo.lock",
      "tooling/conventions/guardrails/rust/rules.json"
    ],
    "input": "the real lockfile and rules.json capabilityCrates; boundary: transitive (not only direct) dependencies, and a failing cargo tree fails the test instead of passing",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Crate //! docs (format and lock contract, discovery deferrals), AGENTS.md Key files rows (toolu-state new, toolu-runtime git), docs/detect.md Rust port section, fixtures/README.md suites and Rust consumers",
    "check": "grep -q 'crates/core/state/src/lib.rs' AGENTS.md && grep -q 'toolu_runtime::git\\|`git` discovery' AGENTS.md && grep -q 'toolu_state::detect' docs/detect.md && grep -q 'gate-bytes.json' fixtures/README.md && grep -q 'edit-records.json' fixtures/README.md && grep -q '^//!.*lock' crates/core/state/src/lib.rs && grep -q '^//!' crates/core/runtime/src/git.rs",
    "ac_refs": [
      "AC-13"
    ],
    "depends_on": [
      "gate-bytes-golden",
      "edit-records",
      "state-cases",
      "sweeper",
      "detect",
      "interleave",
      "deps"
    ],
    "paths": [
      "AGENTS.md",
      "docs/detect.md",
      "fixtures/README.md",
      "crates/core/state/src/lib.rs",
      "crates/core/runtime/src/git.rs"
    ],
    "input": "the final crates and fixtures",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full Rust gate and the TypeScript checks this change touches",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main --title 'feat(state): gate file, telemetry, edit records and git facts without spawning git (#415)' && bun run tooling/src/check-fixture-inventory.ts && bun test packages/toolu-core/src/state/__tests__ packages/toolu-core/src/detect/__tests__ tooling/src/__tests__/check-fixture-inventory.test.ts",
    "ac_refs": [
      "AC-13"
    ],
    "depends_on": [
      "docs"
    ],
    "input": "the whole branch; boundary: coverage floor 90% for toolu-state and 85% for toolu-runtime, zero jscpd clones, unused pub items, file/function/impl size limits, test layout, a fixture case missing from index.json. AC-13's `bun run test` clause runs in Verification against a clean origin/main baseline because this root host fails known environmental cases (comemory be52369e); CI typescript is the authority",
    "model": "inherit"
  }
]
```

## Critical files

- **Create in `toolu-runtime`:**
  - `crates/core/runtime/src/git.rs`, `src/git/{gitdir,defer}.rs`;
  - their tests: `src/tests/git_test.rs`, `src/git/tests/{gitdir,defer}_test.rs`.
- **Modify in `toolu-runtime`:**
  - `crates/core/runtime/src/{lib.rs,host/roots.rs,config/quality.rs,process.rs,process/drain.rs}`;
  - `crates/core/runtime/Cargo.toml` (`nix` `user` feature) and `Cargo.lock`.
- **Create in `toolu-state`:**
  - `crates/core/state/src/{ctx,time,io,lock,git,telemetry,telemetry_schema,gate_schema,js_order,gate_doc,gate_file,edit_records,apply_patch,diff_sha,sweeper,sweep_telemetry,detect}.rs`;
  - `src/git/refs.rs` and `src/detect/{project,tools,lines,read}.rs`;
  - for each, `tests/<module>_test.rs` beside it.
- **Create `toolu-state` integration tests:** `crates/core/state/tests/{helpers/repo.rs,state_cases.rs,gate_bytes.rs,edit_records.rs,git_facts.rs,sweeper.rs,detect.rs,deps.rs,interleave.rs}`.
- **Modify `toolu-state`:** `crates/core/state/{Cargo.toml,src/lib.rs}`.
- **Create fixtures and Bun tests:** `fixtures/state/{gate-bytes,edit-records}.json`, `packages/toolu-core/src/state/__tests__/{gate-bytes,edit-records}.test.ts`.
- **Modify fixtures and docs:** `fixtures/index.json`, `fixtures/README.md`, `tooling/src/__tests__/check-fixture-inventory.test.ts`, `AGENTS.md`, `docs/detect.md`.

## Verification

`cargo xtask gate --base origin/main` is the whole Rust bar:
- fmt and clippy;
- guardrails: sizes, test layout, colocated tests, folders, capabilities, suppression;
- layers;
- coverage: 90% for `toolu-state`, 85% for `toolu-runtime`;
- jscpd, unused pub, docs-cli and cli-compat.

Then the Bun suites for state and detect, plus the fixture inventory. `bun run test` runs last, and its known environment failures on this root host (comemory `be52369e`) are compared against a clean `origin/main` worktree before any is treated as a regression.

**Real inputs:**
- the committed fixtures;
- real `git` repositories, worktrees, submodules and bare repos;
- real child processes (`bun` writers, self-executed Rust writers, `jq`, `git`);
- real files, symlinks and mtimes in temp directories.

**Failure and boundary checks:** each step's `input` lists them (malformed documents, dead and stale locks, PATH emptied, oversized lines, non-UTF-8 diffs, unparseable telemetry).

**Docs synced:** `AGENTS.md`, `docs/detect.md`, `fixtures/README.md`, `fixtures/index.json`, the crate docs.

## Delivery

1. Make scoped commits on `feat/415-toolu-state-gate-file-telemetry` with conventional subjects; the PR is squash-merged.
2. Run `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run docs/toolu/plans/2026-10-06-toolu-state.md --verify` over the whole branch diff.
3. Run `toolu-review:review`, recording the version 2 push-review state.
4. Confirm that `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/verdict.js" status` reports `overall: ready`.
5. Fetch and rebase on `origin/main` if it moved, then re-run the affected checks.
6. Push and open the PR against `main`. Title: `feat(state): gate file, telemetry, edit records and git facts without spawning git (#415)`. The body starts with `Closes Falconiere/toolu#415` and `Part of Falconiere/toolu#402`.
7. Report `pr-open`, then `babysit`, and hand off to `pr-babysit:babysit`.

`TOOLU_PLUGIN_ROOT` is `/root/.claude/plugins/cache/toolu/toolu/7.11.0`, the project install of `toolu@toolu` for this worktree, which holds `plan-ledger.js` and `verdict.js`.

## Deviations

- **runtime-discovery.** The spec listed `core.worktree` and `core.bare` as reasons to ask git. Absorbed submodules always set `core.worktree`, so deferring would have made every submodule spawn git, against AC-6. The walk now applies both keys as git's `setup_discovered_git_dir` and `setup_bare_git_dir` do:
  - `core.bare = true` means no toplevel;
  - otherwise `core.worktree`, resolved against the git dir, is the toplevel.
  
  It still asks git for a linked worktree with `core.worktree`, a quoted or escaped value, includes, `config.worktree`, and a repository format above 1.
- **state-cases.** The 76 Rust-run cases are split over three files to stay under the 300-line limit: `tests/state_cases.rs` (schema, telemetry, branch, diff), `tests/io_cases.rs` and `tests/gate_cases.rs`. The step's check runs all three.
- **edit-records.** The shared `EditRecordSchema` cases need a strict reader, so `toolu_state::edit_records::parse_edit_record` was added. `toolu_state::telemetry::TELEMETRY_EVENTS` lists the closed event set for the `telemetry-events` case.
- **gate-file.** `record_gate_failure` takes a `GateFailure` struct (file, source, reason, violations) rather than four strings, which keeps it within the five-parameter limit.

## Plan review

**Round 1.** AC coverage was checked mechanically from the ledger JSON: AC-1 to AC-13 each have at least one `ac_refs` step, no ref dangles, no step has an empty check, and no `depends_on` points forward or to a missing id. Jev (aggregate) gave fail-on-break 0.39, boundary inputs 0.64 and order 0.52, which led to these findings:

- All checks: 🔴 blocker: `cargo test … 2>&1 | grep 'test result: ok'` takes grep's status. With two `--test` targets, a failing second binary still leaves the first one's `ok` line, so the check passes. Every check now captures the output, requires cargo's exit 0 (`o=$(…) && printf … | grep`), and requires at least one passing test.
- detect: 🟡 should-fix: it is a `toolu-state` module but did not depend on `foundation`, which creates the crate manifest. Added the dependency.
- edit-records / gate-bytes-golden: 🟡 should-fix: both edit `fixtures/index.json` and the inventory count. `edit-records` now depends on `gate-bytes-golden`, which serializes the edits.
- runtime-discovery: 🟡 should-fix: `paths` omitted the fixtures its check reads. Added `fixtures/host/root.json` and `fixtures/config/`.
- state-cases: 🟡 should-fix: `paths` omitted `crates/core/runtime/src/` (`jq_text`), and its input named only the file. The input now lists the rejecting and boundary families it exercises.
- gate: 🔵 consider: added `--title` as CI passes it. AC-13's `bun run test` clause stays in Verification, against an `origin/main` baseline (comemory `be52369e`).

**Round 2.** Jev, per AC, on whether a check fails when the AC breaks: AC-1 0.87, AC-2 0.86, AC-3 0.94, AC-4 0.90, AC-5 0.90, AC-6 0.84, AC-7 0.87, AC-8 0.94, AC-9 0.89, AC-10 0.93, AC-11 0.92, AC-12 0.93, AC-13 0.86. Boundary inputs 0.71, order 0.75. No blocker remains.

**Status:** Approved
