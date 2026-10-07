# Management surface: doctor, config, status, setup agents — Plan

**Date:** 2026-10-07   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-07-management-surface-design.md   **Topic:** #445. `toolu doctor`, `toolu config get|set|validate`, `toolu status`, `toolu setup agents`, and their Markdown.

## Evidence and approach

The approved spec is the contract. No planning classification was left open: step order follows the spec's test files, and a failing report keeps stdout because `crates/cli/src/dispatch.rs` `finish` writes the error envelope only when `stdout` is `None`.

**Already there:**

- `crates/toolu/src/doctor.rs` is the #443 reachability check. `crates/cli/tests/doctor.rs` asserts top-level `reachable`/`path`, the strings `upgrade with: brew upgrade toolu` and the installer command, a `--json` error envelope on failure, a 5s hang bound, a 2s descendant bound, and the npm wrapper. `crates/toolu/src/tests/doctor_test.rs` calls private `reachable`.
- `config`, `status` and `setup` are `Planned` namespaces. `crates/cli/src/tests/links_test.rs` expects `Snapshot` to return `LinkError::NotPorted { issue: 445 }`. `crates/toolu` does not depend on `toolu-state` yet.
- Loader messages live in `crates/core/runtime/src/config/load.rs` (`read_layer`, `envelope_error`). `redact_json` / `redact_text` and `credential_key` are in `config/secrets.rs` and `config/epic.rs`. There is no `Secrets::none` and no public `check_text`.
- `Outcome` fields are public (`crates/core/runtime/src/cli.rs`). `Exit::Usage` is 64, `Blocked` is 2, `Failure` is 1.
- Roots: `Env::process()`, then `env.with("TOOLU_CONFIG_DIR", …)` when `ctx.config_dir` is set, then `Roots::new(env, ctx.host)` — the same construction as `crates/epic-orchestrator/src/lib.rs` `token_new`. Working directory is `toolu_runtime::invocation::current_dir`.
- Claude install record: `crates/core/engine/src/registry/gate.rs` `installed_plugins_path`. Codex listing shape: `crates/core/runtime/src/host/snapshot.rs` `canonical` (`installed[]`, `flag_on`, `pluginId` or `name`+`marketplaceName`). OpenCode record: `packages/toolu-core/src/startup/opencode-status.ts` (`plugins[].name` at `<config root>/toolu/opencode-status.json`).
- Plugin manifest: `toolu_runtime::manifest::read`. Module manifest: `toolu_runtime::registry::manifest::read_manifest`. Listing: `toolu_engine::registry::list_dir`. Compiled rules: `toolu_hub::tool_hook::RULES` (empty until #426–#429, so a valid manifest warns "rule missing").
- State paths: `Roots::project_state_root`, `toolu_state::git::{current_branch, branch_slug}`, `toolu_state::gate_file::read_gate_file`, `toolu_state::detect::tools::tool_available`. Push-review v2 fields are named in `packages/toolu-core/src/gates/push-review.ts`. Waiver files: `packages/toolu-core/src/ledger/push-waiver.ts`.
- Setup oracle: `plugins/toolu/skills/setup/scripts/setup.ts` and `plugins/toolu/skills/__tests__/setup-agents.test.ts`. Five templates under `plugins/toolu/assets/agents/`. OpenCode rewrites that skill by an exact substring in `tools/toolu-opencode/scripts/lib/opencode-port.ts`; `applyEdit` throws unless the substring occurs once. `bun run check:opencode-surface` gates the generated tree.
- `docs/toolu` is gitignored. Design docs are force-added. `cargo` is `$HOME/.cargo/bin/cargo`.

**Constraints:** 300 code lines per file, 50 per function, no `unwrap` in `src`, tests in `tests/<module>_test.rs` beside the module, no `mod.rs` (a `doctor.rs` plus `doctor/`). `toolu-hub` coverage uses the default 85% floor. Plugin crates may call `std::env::current_exe` (the env ban is `var` / `set_var`, not `current_exe`) and must spawn only through `toolu_runtime::process`.

## Workstream summary

Loader helpers → doctor report, including the #443 tests retargeted at the new document → config verbs → redaction across both → status snapshot → setup parity with the Bun script → Markdown, CLI docs, the OpenCode skill port → the Rust gate.

## Steps (machine-readable)

```json
[
  {
    "id": "runtime-helpers",
    "title": "Public config::load::check_text with the loader's exact messages, used by read_layer; Secrets::none() for key-name redaction when secrets.json cannot be loaded",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-runtime --lib check_text_ && t -p toolu-runtime --lib secrets_none_",
    "ac_refs": [],
    "paths": [
      "crates/core/runtime/src/config/load.rs",
      "crates/core/runtime/src/config/tests/load_test.rs",
      "crates/core/runtime/src/config/secrets.rs",
      "crates/core/runtime/src/config/tests/secrets_test.rs"
    ],
    "input": "the text {\"bogus\":1} yields unknown top-level key 'bogus'; a non-object, version 2, and malformed JSON match read_layer. Secrets::none() redacts a credential-named key's value to <redacted> and leaves a canary in a normal string",
    "model": "inherit"
  },
  {
    "id": "doctor-report",
    "title": "toolu doctor checks (binary, reachability, runtime, host, config, plugins, skew, registry, tools), redacted report on stdout even when exit is 1, #443 tests retargeted, doctor_report covering AC-1, AC-2's doctor half, AC-5, AC-8, AC-9",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-hub --lib doctor:: && t -p toolu-cli --test doctor && t -p toolu-cli --test doctor_report",
    "ac_refs": ["AC-1", "AC-2", "AC-5", "AC-8", "AC-9"],
    "depends_on": ["runtime-helpers"],
    "paths": [
      "crates/toolu/Cargo.toml",
      "crates/toolu/src/doctor.rs",
      "crates/toolu/src/doctor/checks.rs",
      "crates/toolu/src/doctor/binary.rs",
      "crates/toolu/src/doctor/inventory.rs",
      "crates/toolu/src/doctor/skew_check.rs",
      "crates/toolu/src/doctor/registry_check.rs",
      "crates/toolu/src/doctor/tools.rs",
      "crates/toolu/src/doctor/config_check.rs",
      "crates/toolu/src/doctor/tests/checks_test.rs",
      "crates/toolu/src/doctor/tests/binary_test.rs",
      "crates/toolu/src/doctor/tests/inventory_test.rs",
      "crates/toolu/src/doctor/tests/skew_check_test.rs",
      "crates/toolu/src/doctor/tests/registry_check_test.rs",
      "crates/toolu/src/doctor/tests/tools_test.rs",
      "crates/toolu/src/doctor/tests/config_check_test.rs",
      "crates/toolu/src/tests/doctor_test.rs",
      "crates/cli/tests/doctor.rs",
      "crates/cli/tests/doctor_report.rs",
      "crates/cli/src/commands.schema.json",
      "Cargo.lock"
    ],
    "input": "temp HOME per host (--host claude|codex|opencode) with copied plugins/toolu and plugins/pr-babysit; a codex stub printing plugin list --json; opencode-status.json names only and skew summary not applicable. Project config {\"bogus\":1} makes doctor exit 1 and the config check contain unknown top-level key 'bogus'. plugin.json version 0.0.1 warns skew and exits 0; hookProtocol 99 fails and exits 1. PATH without gh fails tools with pr-babysit needs gh. pre-tools.d: a third-party .js warns, an orphan manifest warns, version 2 fails, a .sh module does not warn. The existing doctor.rs sandbox (copied binary, hanging script, npm wrapper) stays, with HOME isolated",
    "model": "inherit"
  },
  {
    "id": "config-cli",
    "title": "toolu config get, set and validate through the loader, atomic writes, refusals that leave the file byte-identical",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-hub --lib config:: && t -p toolu-cli --test config_cli",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["runtime-helpers"],
    "paths": [
      "crates/toolu/src/config.rs",
      "crates/toolu/src/config/edit.rs",
      "crates/toolu/src/config/show.rs",
      "crates/toolu/src/config/tests/edit_test.rs",
      "crates/toolu/src/config/tests/show_test.rs",
      "crates/toolu/src/tests/config_test.rs",
      "crates/cli/tests/config_cli.rs",
      "crates/cli/src/commands.schema.json"
    ],
    "input": "temp user and project toolu.config.json. Unknown key {\"bogus\":1} makes validate exit 1 with the loader message. config set version 2 and config set epic.statusToken x exit 1 with the same sha256. config set gates.pushReview '\"off\"' round-trips through get. A missing file is created. A missing gates object is created. A present non-object intermediate is refused. get of a missing key exits 1 with empty stdout",
    "model": "inherit"
  },
  {
    "id": "redaction",
    "title": "config get and doctor redact canary values and credential values; a refused secrets.json prints no canary",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-cli --test redaction",
    "ac_refs": ["AC-6"],
    "depends_on": ["doctor-report", "config-cli"],
    "paths": [
      "crates/toolu/src/doctor.rs",
      "crates/toolu/src/doctor/config_check.rs",
      "crates/toolu/src/config/show.rs",
      "crates/cli/tests/redaction.rs"
    ],
    "input": "secrets.json mode 0600 with a canary, epic.label containing that canary, epic.statusToken set to a second secret. Text and --json, stdout and stderr, of config get and doctor. Boundary: secrets.json mode 0644",
    "model": "inherit"
  },
  {
    "id": "status-snapshot",
    "title": "StatusSnapshot::snapshot and toolu status --json for the repository, gate file, push-review document and waivers",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-hub --lib status:: && t -p toolu-cli --lib statusline_gets_the_hubs_status_snapshot && t -p toolu-cli --test status_cli",
    "ac_refs": ["AC-10"],
    "depends_on": ["runtime-helpers"],
    "paths": [
      "crates/toolu/Cargo.toml",
      "crates/toolu/src/status.rs",
      "crates/toolu/src/status/repo.rs",
      "crates/toolu/src/status/gate.rs",
      "crates/toolu/src/status/tests/repo_test.rs",
      "crates/toolu/src/status/tests/gate_test.rs",
      "crates/toolu/src/tests/status_test.rs",
      "crates/cli/src/tests/links_test.rs",
      "crates/cli/tests/status_cli.rs",
      "crates/cli/src/commands.schema.json"
    ],
    "input": "git init repository on branch feat/x, one commit, one edited file, a gate file from record_gate_failure with two entries, push-review JSON and waiver JSON under project_state_root. Boundary: no git toplevel and no project root; TOOLU_PROJECT_DIR set on a non-repo",
    "model": "inherit"
  },
  {
    "id": "setup-agents",
    "title": "toolu setup agents preview, install and remove, byte-identical to bun setup.ts except the program prefix",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; t() { o=$(cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-hub --lib setup:: && t -p toolu-cli --test setup_agents_parity",
    "ac_refs": ["AC-4"],
    "paths": [
      "crates/toolu/src/setup.rs",
      "crates/toolu/src/setup/agents.rs",
      "crates/toolu/src/setup/tests/agents_test.rs",
      "crates/toolu/src/tests/setup_test.rs",
      "crates/cli/tests/setup_agents_parity.rs",
      "plugins/toolu/assets/agents/quick-task.toml",
      "plugins/toolu/assets/agents/deep-explore.toml",
      "plugins/toolu/assets/agents/research-agent.toml",
      "plugins/toolu/assets/agents/implementer.toml",
      "plugins/toolu/assets/agents/architect.toml",
      "plugins/toolu/skills/setup/scripts/setup.ts"
    ],
    "input": "the five real templates, twin CODEX_HOME sandboxes, TOOLU_TIMESTAMP fixed. Profiles: fresh home, managed outdated files, unmanaged conflict. Flags: preview, install, install --force, remove, remove --yes, remove --yes --force. Boundaries: conflict without --force, remove without --yes, OpenCode host, invalid template dir, existing backup, CODEX_HOME and HOME unset",
    "model": "inherit"
  },
  {
    "id": "markdown-docs",
    "title": "Commands, setup skill, OpenCode port, generated CLI docs, schema, install and config docs, plugin README, AGENTS.md",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask docs-cli --check && cargo xtask check-markdown-cli && cargo test -q -p toolu-cli --test contract && bun run check:opencode-surface && grep -q 'toolu doctor' plugins/toolu/skills/setup/SKILL.md && grep -q 'toolu config validate' plugins/toolu/skills/setup/SKILL.md && grep -q 'toolu setup agents' plugins/toolu/skills/setup/SKILL.md",
    "ac_refs": ["AC-7"],
    "depends_on": ["doctor-report", "config-cli", "status-snapshot", "setup-agents"],
    "paths": [
      "plugins/toolu/commands/doctor.md",
      "plugins/toolu/commands/status.md",
      "plugins/toolu/skills/setup/SKILL.md",
      "plugins/toolu/README.md",
      "tools/toolu-opencode/scripts/lib/opencode-port.ts",
      "tools/toolu-opencode/generated/skills/toolu-setup/SKILL.md",
      "docs/cli",
      "docs/install.md",
      "docs/config.md",
      "AGENTS.md",
      "crates/cli/src/commands.schema.json",
      "crates/cli/tests/contract.rs"
    ],
    "input": "the new command Markdown and the rewritten setup skill, checked by cargo xtask check-markdown-cli. The OpenCode port's from-string is the new skill paragraph; the replacement still says OpenCode has nothing to install. cargo xtask docs-cli regenerates docs/cli, and INSTA_UPDATE=always cargo test -p toolu-cli --test contract accepts the commands snapshot before the check runs without INSTA_UPDATE",
    "model": "inherit"
  },
  {
    "id": "full-gate",
    "title": "cargo xtask gate for the #445 title, plus the Markdown–CLI drift check",
    "check": "export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(toolu): doctor, config, status and setup agents (#445)' && cargo xtask check-markdown-cli",
    "ac_refs": [],
    "depends_on": ["markdown-docs", "redaction"],
    "paths": [
      "crates/",
      "Cargo.toml",
      "Cargo.lock",
      "docs/",
      "plugins/toolu/",
      "AGENTS.md",
      "tools/toolu-opencode/"
    ],
    "input": "the whole workspace diff against origin/main",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/runtime/src/config/{load,secrets}.rs` and their `tests/`
- `crates/toolu/Cargo.toml` (`toolu-state`, dev `tempfile`)
- `crates/toolu/src/doctor.rs` plus `doctor/{checks,binary,inventory,skew_check,registry_check,tools,config_check}.rs` and `doctor/tests/`
- `crates/toolu/src/config.rs` plus `config/{edit,show}.rs` and `config/tests/`
- `crates/toolu/src/status.rs` plus `status/{repo,gate}.rs` and `status/tests/`
- `crates/toolu/src/setup.rs` plus `setup/agents.rs` and `setup/tests/`
- `crates/cli/tests/{doctor,doctor_report,config_cli,redaction,status_cli,setup_agents_parity}.rs`
- `crates/cli/src/tests/links_test.rs`, `crates/cli/src/commands.schema.json`
- `plugins/toolu/commands/{doctor,status}.md`, `plugins/toolu/skills/setup/SKILL.md`, `plugins/toolu/README.md`
- `tools/toolu-opencode/scripts/lib/opencode-port.ts` and the generated setup skill
- `docs/cli/**`, `docs/install.md`, `docs/config.md`, `AGENTS.md`
- `docs/toolu/{brainstorms,specs,plans}/2026-10-07-management-surface*.md`, force-added

## Behavior the steps must not reinterpret

- **Doctor outcome.** Construct `Outcome { exit, stdout: Some(report), stderr: Some(count line) }`. Do not call `Outcome::failed` for a finished report. Stderr is `toolu doctor: <n> check(s) failed` only when a check failed. Warn does not change the exit.
- **#443 tests.** Isolate `HOME` and do not read the machine's plugins or config. Point `reachable`/`path` at the reachability check's `details`. The binary summary still contains the upgrade command (`BREW_UPGRADE` or `INSTALLER`). A missing or non-native `toolu` is exit 1 with the full report on stdout, not `document["error"]["code"]`. The hang test still finishes in under 5s: reachability, `bun --version` and the shell probe use a 1s deadline. The Codex listing uses its own `process::Spec` with a 10s deadline and runs only when the host is Codex; do not call `codex_plugin_list`, which has no deadline. A timeout is warn or, for reachability, fail — never a hang. Sandboxes pass `--host`. Empty plugin inventory warns and does not fail. Zero plugins makes skew `ok`.
- **Inventory.** Claude: `*@toolu` entries, scope `user`, plus `project` or `local` whose `projectPath` is the project root; `installPath` is the root passed to `manifest::read`. An entry without `installPath` warns and is omitted from skew. Codex: `marketplaceName == "toolu"`, installed and enabled by `flag_on`, root `source.path`. OpenCode: `plugins[].name` only; skew summary `not applicable`, status `ok`. Cursor and Hermes: plugins warn. An unreadable `plugin.json` is `Skew::Mismatch`.
- **Registry.** Walk `pre-tools.d` and `post-tools.d` with `list_dir`. `.sh` is ok. `.js` warns with the Bun-bridge sentence. Unnamespaced files warn. A manifest `read_manifest` rejects, including version other than 1, fails. A valid manifest whose rule is not in `RULES` warns. Orphan means `plugin_presence` is `Absent`, not `Unknown`.
- **Tools.** The spec's table. Status `fail` if any required tool is missing, else `warn` if any optional tool is missing. Summary `<plugin> needs <tool>`, required first, joined with `; `.
- **Config.** `get` prints the redacted merged document and does not fail only because `epic::parse` fails. A value that parses as JSON is that value; otherwise it is a string. Writes use `toolu_runtime::atomic::write_atomic`. Empty dotted segment is `Exit::Usage`.
- **Secrets.** Loaded secrets: `redact_json` on the JSON report and `redact_text` on the text report. The key `statusToken` may appear; its value and the canary may not. Doctor config failure summary is the epic path error. `merged` is included only when `secrets::load` succeeds. A refused secrets file: doctor fails that check, omits `merged`, and `config get` exits 1 with empty stdout.
- **Status.** `git status --porcelain=v1 --branch` with a deadline through `process::run`. `??` untracked; non-space non-`?` index column staged; the same on the work-tree column unstaged. No git toplevel: empty repo fields. Gate is read only when `project_root` exists. Push review and waivers need `branch_slug` (`feat/x` → `feat_x`). `recorded` copies the parsed file into `document`. `links_test` stops expecting `NotPorted`.
- **Setup.** Embed the five templates with `include_str!`. `TOOLU_AGENT_TEMPLATE_DIR`, `CODEX_HOME`, `HOME` and `TOOLU_TIMESTAMP` mean what they mean in `setup.ts`. Compare stdout after replacing a leading `setup.ts` with `toolu setup agents`. `REFUSED` lines stay as the script prints them. OpenCode and confirmation refusals are `Exit::Blocked`. Do not delete `setup.ts` or `setup-agents.test.ts`.
- **Docs.** `/toolu:doctor` and `/toolu:status` run `toolu doctor` and `toolu status` and present the output. The setup skill runs `toolu setup agents`, then `toolu doctor` and `toolu config validate`. Update the OpenCode port's source substring and regenerate the surface. `docs/install.md` drops "other checks arrive in #445". Add a `crates/toolu` row to the AGENTS.md key-files table. `$defs/doctor` is replaced; `$defs/config`, `$defs/status` and `$defs/setupAgents` are added.

## Verification

- **End to end:** the six CLI test binaries pass on real temp homes, real plugin trees, a real `git init` repository and the real agent templates. `Snapshot::snapshot` and `toolu status --json` return the same document.
- **Failure and boundary:** the cases named in each step's `input`, including malformed JSON, a non-object `set` path, `secrets.json` mode 0644, no project root, an OpenCode setup refusal, and the #443 hang and wrapper probes.
- **Docs:** `cargo xtask docs-cli --check`, `cargo xtask check-markdown-cli`, `bun run check:opencode-surface`, and the contract snapshot.

## Delivery

1. Before each push: `git fetch origin`, and rebase on `origin/main` if it moved. Re-run the affected step checks.
2. Commit scoped work as it lands. Force-add `docs/toolu`. The PR title is `feat(toolu): doctor, config, status and setup agents (#445)`.
3. Run `bun plugins/toolu/hooks/dist/plan-ledger.js run docs/toolu/plans/2026-10-07-management-surface.md --verify` so every step is fresh-green against the final diff.
4. Run `toolu-review:review` on the committed branch diff. Then `bun plugins/toolu/hooks/dist/verdict.js status` must report `overall: ready`.
5. Push `feat/445-management-surface-doctor-config-and`. Open the PR against `main` with a body that starts `Closes Falconiere/toolu#445` and `Part of Falconiere/toolu#402`. Verify the number and the head and base. Then hand off to `pr-babysit:babysit`.

## Plan review

**Status:** Approved

AC-1 through AC-10 each have a step. Dependencies point at earlier steps. Every behavior step names a real input and the observable the spec requires, including the invalid and boundary cases. Docs and delivery are steps, not follow-ups. Jev scored the doctor inputs against AC-1, AC-5, AC-8 and AC-9 at 0.74. The AC-2 split (doctor exit in `doctor-report`, validate exit in `config-cli`) was uncertain until the doctor input stated the loader message and exit 1 explicitly.
