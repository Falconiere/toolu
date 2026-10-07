# Management surface: doctor, config, status, setup agents — Design

**Date:** 2026-10-07   **Status:** Approved   **Author:** Claude (epic worker, #445)   **Topic:** `toolu doctor`, `toolu config get|set|validate`, `toolu status`, `toolu setup agents`, and their Markdown

## Problem

There is no single place where a user or an agent can find out why toolu does not behave. The #443 `toolu doctor` answers one question, whether the agent's shell finds the binary. Version skew, broken config, orphaned registry manifests, missing `gh` or `herdr` and the installed plugin set are each found by hand today. `toolu config`, `toolu status` and `toolu setup` are placeholders, and the setup skill runs a Bun script that the native binary is meant to replace (#402).

## Non-Goals

1. Rendering the statusline (#431). `toolu status` provides the data, and Jev readiness, the comemory count and the OpenCode startup record stay with that port.
2. Fixing anything. `doctor` reports and gives hints. It never writes.
3. Porting the gates, the ledger or push waivers (#419–#424). `status` only reads their state files.
4. Deleting `plugins/toolu/skills/setup/scripts/setup.ts` or its Bun test. The script stays as the parity oracle until Bun is retired (#425/#440). Only the skill stops calling it.
5. OpenCode's native skew. OpenCode runs plugins through the TypeScript adapter until #437, so the skew check reports "not applicable" there.
6. Validating every config section's contents. `config set` and `validate` enforce what the loader enforces: the envelope, plus the `epic` section through `config::epic::parse`.

## Architecture

Everything lives in the hub, `crates/toolu` (package `toolu-hub`). It composes existing core parts and adds two small runtime helpers.

- **Runtime additions (`toolu-runtime`).**
  - `config::load::check_text(text) -> Result<Map, String>`: the loader's per-file rule (malformed JSON, top level not an object, unknown top-level key, version other than 1), with the loader's exact messages. `read_layer` calls it, so there is one rule.
  - `config::secrets::Secrets::none()`: an empty set. It is used when `secrets.json` cannot be loaded, so key-name redaction still applies.
- **`toolu doctor`** (`crates/toolu/src/doctor.rs` plus `doctor/`): a list of checks. Each check returns `Check { id, status: Ok|Warn|Fail, summary, hint: Option, details: Value }`. The report is built once, redacted once with `redact_json` (JSON) or `redact_text` (text) using `secrets::load(&roots)`, or `Secrets::none()` when that fails, and printed. The exit is 1 (`Exit::Failure`) when any check fails, else 0. The report is on stdout in both cases, and a failing run adds `toolu doctor: <n> check(s) failed` on stderr.
- **Plugin inventory** (`doctor/inventory.rs`), one source per host:
  - Claude: `installed_plugins.json` (path as in `engine::registry::gate`). Keep the `*@toolu` entries whose `scope` is `user`, plus `project` or `local` entries whose `projectPath` is the current project root. Read each `installPath`'s `plugin.json` through `manifest::read`.
  - Codex: `process::commands::codex_plugin_list(env)`, keeping the `marketplaceName == "toolu"` entries that are installed and enabled. Each `source.path` gives the root.
  - OpenCode: `<config root>/toolu/opencode-status.json`, with plugin names only.
  - Cursor and Hermes: unknown, so the check warns.
- **Required tools** (`doctor/tools.rs`): a static table from plugin name to `(tool, Required|Optional, hint)`. Presence comes from `toolu_state::detect::tools::tool_available` (a PATH scan, nothing spawned).

  | Plugin | Required | Optional |
  |---|---|---|
  | `toolu` | `git` | — |
  | `pr-babysit` | `gh`, `git` | `herdr` |
  | `epic-orchestrator` | `gh`, `git`, `herdr` | — |
  | `toolu-review` | `git` | `gh` |
  | `ast-grep` | `ast-grep` | — |
  | `python-quality` | — | `python3`, `ast-grep` |
  | `rust-quality` | — | `cargo`, `ast-grep` |
  | `ts-quality` | — | `node` |

  Bun is reported by the `runtime` check, not this table.
- **`toolu config`** (`config.rs` plus `config/`): `get`, `set` and `validate` over `config::load::{load, config_files, check_text, merge}` and `config::epic::parse`. Writes go through `atomic::write_atomic`.
- **`toolu status`** (`status.rs` plus `status/`): `Snapshot` implements `StatusSnapshot`. Git facts come from `toolu_state::git::current_branch`, plus one bounded `git status --porcelain --branch` through `process::run`. Gate state comes from `toolu_state::gate_file::read_gate_file`, and the push-review and waiver files are read as JSON. `toolu status` prints the same document that statusline (#431) receives through `crates/cli/src/links.rs`.
- **`toolu setup agents`** (`setup.rs` plus `setup/`): a port of `setup.ts`. The templates are `include_str!` of `plugins/toolu/assets/agents/<name>.toml`. `TOOLU_AGENT_TEMPLATE_DIR`, when set, reads the five files from that directory instead and validates them with the script's rules. `CODEX_HOME`, `HOME` and `TOOLU_TIMESTAMP` mean what they mean for the script. Output lines and file effects are byte-identical, and so are exit codes, with refusals mapped to `Exit::Blocked` (2) as the script's 2.
- **Markdown.**
  - `plugins/toolu/commands/doctor.md` and `status.md` run `toolu doctor` and `toolu status` and present their output.
  - `plugins/toolu/skills/setup/SKILL.md` runs `toolu setup agents …`, then `toolu doctor` and `toolu config validate`.
  - `cargo xtask check-markdown-cli` covers all three.

The decisive trade-off is composition over new mechanisms. Every fact comes from the reader that enforcement already uses (loader, skew rule, manifest reader, plugin gate, gate file), so `doctor` cannot disagree with the hooks.

## Interfaces / Schema

```text
toolu doctor                                   # report; exit 1 if any check fails
toolu config get [<key>]                       # merged value at dotted <key>, whole config without it
toolu config set <key> <value> [--project]     # <value> is JSON, else a string; user file unless --project
toolu config validate                          # exit 1 with the loader's message when invalid
toolu status                                   # repository and gate state for the current directory
toolu setup agents preview
toolu setup agents install [--force]
toolu setup agents remove [--yes] [--force]
```

Checks, in order, by `id`:

| id | ok | warn | fail |
|---|---|---|---|
| `binary` | version, `hookProtocol`, path, `install: homebrew\|installer`, `upgrade` command | — | — |
| `reachability` | `sh -c 'command -v toolu'` finds the native binary (the #443 probe) | — | not found, or shadowed |
| `runtime` | `bun --version` (from `TOOLU_BUN`, else `PATH`) | Bun missing; hint to install Bun 1.4.x | — |
| `host` | host, config root, project root | `Roots::warning()` | — |
| `config` | files read, valid | loader warnings (malformed JSON ignored) | invalid envelope, `epic` error or `secrets.json` refused, with the loader's message |
| `plugins` | name, version and root of each installed toolu plugin | inventory unavailable, or no plugin installed | — |
| `skew` | same version and protocol. On OpenCode: status `ok`, summary `not applicable` | `Skew::Advise` | `Skew::Mismatch` |
| `registry` | no finding. A `.sh` module is ok (it keeps the executable contract) | orphaned manifest (its plugin is absent), rule missing from this binary, a `.js` module ("runs through the Bun bridge until #440"), or an unnamespaced file | a manifest that is invalid or has the wrong version (its rule never runs) |
| `tools` | every tool present | an optional tool missing | a required tool missing, with an install hint |

`toolu doctor --json` (replaces `$defs/doctor`):

```json
{"namespace":"doctor","ok":false,
 "checks":[{"id":"tools","status":"fail","summary":"pr-babysit needs gh","hint":"install the GitHub CLI: https://cli.github.com","details":{"missing":[{"plugin":"pr-babysit","tool":"gh","required":true}]}}]}
```

Text: one line per check, `<ok|warn|fail> <id>: <summary>`, then `  fix: <hint>` when there is a hint. The `tools` summary lists every missing tool as `<plugin> needs <tool>`, required tools first, joined with `; `. Status is `fail` when any required tool is missing, otherwise `warn` when any optional tool is missing. The hint is the first missing required tool's hint, otherwise the first missing optional tool's hint.

Check `details` that tests assert:

- `binary`: `version`, `hookProtocol`, `path`, `install` (`homebrew` when `upgrade_command` is the brew command, otherwise `installer`), `upgrade`.
- `reachability`: `reachable` and `path`. `shadowed` is true when `command -v toolu` prints a path whose `--hook-protocol` is not a positive integer (the npm wrapper). Not found: `reachable` false, `path` null, `shadowed` false.
- `plugins`: `plugins`, an array of `{name, version, root}` in inventory order. OpenCode entries are `{name}` only.
- `config`: `files` (the paths read) and, when `secrets::load` succeeds, `merged` (the redacted merged document). When secrets cannot be loaded, `merged` is omitted.

`toolu config get --json`: `{"namespace":"config","key":<string|null>,"value":<json>,"files":{"user":<path>,"project":<path|null>}}`. `set --json`: `{"namespace":"config","key":…,"value":…,"file":<path>}`. `validate --json`: `{"namespace":"config","valid":true,"files":{…},"warnings":[…]}`. Text `get` prints the value as pretty JSON, and a string value prints raw. Dotted keys split on `.`, and an empty segment is a usage error (64). A missing key exits 1 with stderr `toolu config get: no such key '<key>'` and empty stdout. `get` prints the redacted merged loader document and does not fail only because `epic::parse` would fail. Absent objects along a `set` path are created; a present non-object intermediate is refused.

`toolu status --json`, the same as `StatusSnapshot::snapshot`:

```json
{"namespace":"status","host":"claude","cwd":"/r","repo_root":"/r","branch":"feat/x","ahead":0,"behind":1,
 "working_tree":{"staged":0,"unstaged":2,"untracked":1},
 "gate":{"status":"failing","reason":"…","entries":[{"file":"src/a.ts","source":"ts-quality","reason":"…","violations":"…","updatedAt":"…"}]},
 "push_review":{"path":"/r/.claude/tmp/push-review/feat_x.json","state":"recorded","document":{"version":2,"diff_sha":"…","review_round":1,"reviewers":["code-review"],"findings_count":0,"reviewed_files":["src/a.ts"]}},
 "waivers":{"waiver":null,"pending":null}}
```

- State files use `Roots::project_state_root`: the gate file is `<state>/quality-gate-status.json`; push review is `<state>/push-review/<slug>.json`; waivers are `<state>/push-review/<slug>.waiver.json` and `<slug>.pending-waiver.json`. `<slug>` is `toolu_state::git::branch_slug` (`feat/x` becomes `feat_x`).
- `gate.status` is `passing`, `failing`, `missing` or `invalid` (`Malformed` and `Unrecognized` are `invalid`, with the reader's reason). `entries` is the failing slots, file key included, `violations` kept; otherwise `[]`.
- `push_review.state` is `missing`, `recorded` or `invalid`. `recorded` copies the parsed file into `document` (real v2 names: `review_round`, `findings_count`, `reviewed_files` as the path array). `invalid` has `reason` and `document` null. Each waiver is the file's object, or `null` when missing or unreadable.
- `git status --porcelain=v1 --branch` supplies ahead, behind and the working tree. `??` counts as untracked; a non-space, non-`?` index column counts as staged; the same on the work-tree column counts as unstaged (`MM` counts as both). A missing ahead or behind side is 0.
- No git toplevel: `repo_root` and `branch` are `""`, and ahead, behind and the working-tree counts are 0. The gate is still read when `project_root` exists (`TOOLU_PROJECT_DIR` or the host project variable). Push review and waivers stay missing without a branch. When `project_root` is also absent, `gate.status` and `push_review.state` are `missing` and both waivers are null.

## Failure modes and edge cases

- **`doctor` with no toolu on PATH.** The `reachability` check fails, the full report is still printed, and the exit is 1. In the #443 contract the report was missing in that case; now the text report is on stdout and the reason on stderr.
- **A hanging probe.** `bun --version`, `codex plugin list` and the shell probe all run through `process::run` with a deadline (1 s, 10 s and 1 s). A timeout reads as missing (warn), never as a hang.
- **Unreadable `installed_plugins.json` or `plugin.json`.**
  - Plugins: an inventory problem warns.
  - Skew: an unreadable `plugin.json` is `Skew::Mismatch`, so it fails, per #411.
- **Config.**
  - Invalid envelope: `doctor`'s config check fails with `<path>: <reason>` and `validate` exits 1 with the same text.
  - Malformed JSON: the loader ignores it, so `doctor` warns. `validate` exits 1, because the file is unusable.
  - `config set` on a malformed or invalid target refuses (exit 1) and writes nothing.
  - `config set` producing an envelope or `epic` error refuses with that message, and the file stays byte-identical.
  - A credential-named key under `epic` is refused, with a message naming the path and never the value.
  - A missing target file is created as `{"version":1, …}` along with its directory.
  - Absent object intermediates on the dotted path are created. A present non-object intermediate is refused.
- **Secrets.** `secrets.json` refused (wrong mode, symlink, malformed):
  - `doctor`'s config check fails with the secret error (it contains no bytes). `merged` is omitted, and key-name redaction still runs, so a credential value is not printed. A canary that lived only in a non-credential string is absent because that document is not printed.
  - `config get` refuses (exit 1) with the secret error and empty stdout, because it cannot prove its output is clean.
- **A loaded secret.** `redact_json` keeps a credential-named key and replaces its value with `<redacted>`. It also replaces canary substrings. `epic::parse` still fails the doctor config check with `epic.statusToken: credential belongs in toolu/secrets.json` (the path names the key; the value is never in the message). `config get` still prints the redacted merged document.
- **Status.**
  - No git toplevel: empty repository fields, as in the schema. The gate is read from `project_state_root` when a project root exists, and is `missing` when it does not. There is no separate cwd `.claude/tmp` probe.
  - An invalid gate file is `invalid`, with the reason.
  - `git status` failing or timing out: ahead, behind and the working-tree counts are 0. `branch` still comes from `current_branch`.
- **`setup agents`.**
  - An invalid template directory: exit 1 with `toolu setup agents: invalid agent template: <file>`.
  - OpenCode host: exit 2 with the script's refusal text.
  - `remove` without `--yes`, or a conflict without `--force`: exit 2 with the script's `REFUSED …` line.
  - An existing backup file: exit 1.
  - `CODEX_HOME` and `HOME` both unset: exit 1.
- **Concurrency.** `config set` writes a temp file and renames it (`write_atomic`). Two writers race last-writer-wins, as with any editor. Nothing else writes.

## Acceptance criteria

- **AC-1:** On a Claude, a Codex and an OpenCode sandbox (temp `HOME` with real plugin trees copied from `plugins/`), `toolu doctor --json` emits one document valid against `$defs/doctor` with exactly the checks `binary, reachability, runtime, host, config, plugins, skew, registry, tools`. The plugins check's `details.plugins` lists `{name, version, root}` for each installed toolu plugin. OpenCode entries are `{name}` only, and its `skew` check is `ok` with summary `not applicable`.
- **AC-2:** Given a project config with an unknown top-level key, `toolu doctor` exits 1 and its `config` check fails with the loader's message (`<path>: unknown top-level key 'bogus'`). `toolu config validate` exits 1 with the same message.
- **AC-3:** `toolu config set` refuses a value that would make the target invalid (`config set version 2`, `config set epic.statusToken x`). It exits 1 and leaves the file byte-identical. A valid `config set gates.pushReview '"off"'` writes the user file (the project file with `--project`), and `config get gates.pushReview` returns `"off"`.
- **AC-4:** For `preview`, `install`, `install --force`, `remove`, `remove --yes` and `remove --yes --force`, `toolu setup agents` and `bun setup.ts` give the same stdout lines, exit code and resulting `agents/` tree from identical sandboxes over the five profiles. The cases are a fresh `CODEX_HOME`, managed outdated files, and an unmanaged conflict. Error text matches except for the program prefix.
- **AC-5:** A plugin whose `plugin.json` has the binary's `hookProtocol` and a different version makes `doctor` warn on `skew` and exit 0. A different `hookProtocol` makes it fail with exit 1.
- **AC-6:** A canary value in `secrets.json` (0600), also planted in `epic.label`, does not appear in the stdout or stderr of `toolu config get` or `toolu doctor`, in text or `--json`. `config get` prints `<redacted>` in place of that canary and in place of the `epic.statusToken` value, and the key name `statusToken` may remain. `doctor` fails the `config` check with `epic.statusToken: credential belongs in toolu/secrets.json` and its redacted `merged` details also show `<redacted>` for both. The canary value and the token value are absent from every one of those outputs.
- **AC-7:** `/toolu:doctor` and `/toolu:status` exist in `plugins/toolu/commands/`, run `toolu doctor` and `toolu status`, and `cargo xtask check-markdown-cli` passes on them and on the rewritten setup skill.
- **AC-8:** With `pr-babysit` installed and no `gh` on `PATH`, `toolu doctor` fails the `tools` check with `pr-babysit needs gh` and an install hint, and exits 1.
- **AC-9:** A third-party `.js` module in `pre-tools.d` makes the `registry` check warn that it runs through the Bun bridge until #440. An orphaned manifest (its plugin absent from `installed_plugins.json`) warns. A manifest with `"version": 2` fails. A namespaced `.sh` module does not warn.
- **AC-10:** In a real git repository with a failing gate file (two entries), a push-review state and a waiver, `toolu status --json` and `Snapshot::snapshot` return the same document with those values, valid against `$defs/status`.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Temp `HOME` per host; Claude `installed_plugins.json` pointing at copied `plugins/toolu` and `plugins/pr-babysit`; Codex: a `codex` executable on `PATH` printing a real `plugin list --json` listing (the snapshot tests' pattern); OpenCode `opencode-status.json` | Schema-valid report, nine checks in order, `details.plugins` versions; OpenCode names only and skew `not applicable` | Cursor host: inventory warns | `cargo test -p toolu-cli --test doctor_report` |
| AC-2 | Project `.claude/toolu.config.json` `{"bogus":1}` | Exit 1, message in `config` check and `validate` | Malformed JSON: doctor warns, validate exits 1; `config get` of a missing key exits 1 | `doctor_report`, `cargo test -p toolu-cli --test config_cli` |
| AC-3 | Real user and project files in a temp repository | Refusal exit 1 with an identical `sha256` of the file; valid set round-trips | Missing file is created; absent `gates` object is created; a present non-object intermediate is refused | `config_cli` |
| AC-4 | Five real templates, twin `CODEX_HOME`s, `TOOLU_TIMESTAMP` fixed | Identical stdout, exit code and tree (bytes and mode) | Conflict without `--force`; remove without `--yes`; OpenCode refusal | `cargo test -p toolu-cli --test setup_agents_parity` (runs `bun`) |
| AC-5 | Copied plugin with edited `plugin.json` (version `0.0.1`; then `hookProtocol` 99) | Warn and exit 0; fail and exit 1 | Unreadable `plugin.json` fails | `doctor_report` |
| AC-6 | `secrets.json` 0600 with a canary, `epic.label` containing it, `epic.statusToken` set to a second secret | Canary and token value absent from get and doctor, text and `--json`, stdout and stderr; `<redacted>` present in get and in doctor `merged` | `secrets.json` 0644: config check fails, `merged` omitted, `config get` refuses, canary still absent | `cargo test -p toolu-cli --test redaction` |
| AC-7 | The new Markdown files | Drift gate exit 0 | A misspelled verb fails the gate (existing gate tests) | `cargo xtask check-markdown-cli` |
| AC-8 | Claude sandbox with pr-babysit, `PATH` without `gh` | `tools` fails, hint, exit 1 | `herdr` missing only warns | `doctor_report` |
| AC-9 | Real `pre-tools.d` with `x@vendor__mod.js`, `jev@toolu__rule.json` (jev absent), a version 2 manifest, and `x@vendor__mod.sh` | warn, warn, fail; the `.sh` module adds no warning | Unnamespaced file warns | `doctor_report` |
| AC-10 | `git init` repository, a commit, an edited file, gate file written by `toolu_state::gate_file::record_gate_failure`, push-review and waiver JSON under `project_state_root` | Fields as written, including `violations`, `review_round`, `findings_count` and the `reviewed_files` paths; path slug `feat_x` for branch `feat/x` | No git toplevel and no project root: empty repo fields, gate and push review `missing`. `TOOLU_PROJECT_DIR` set on a non-repo: gate still read, push review `missing` | `cargo test -p toolu-cli --test status_cli`, hub unit tests |

Unit tests beside each module (`crates/toolu/src/**/tests/*_test.rs`) cover the pure parts: the tool table, the check folding into an exit, the dotted-path set, the snapshot readers and the template validation.

## Documentation impact

- `docs/cli/**`, regenerated (`cargo xtask docs-cli`), and the `commands --json` snapshot.
- `crates/cli/src/commands.schema.json`: `$defs/doctor` replaced; `$defs/config`, `$defs/status` and `$defs/setupAgents` added.
- `docs/install.md`: the doctor paragraph, whose "other checks arrive in #445" is now done.
- `docs/config.md`: a "CLI" section for `toolu config`.
- `plugins/toolu/README.md`: the commands `/toolu:doctor` and `/toolu:status`, and the setup skill.
- `plugins/toolu/skills/setup/SKILL.md`, rewritten.
- `AGENTS.md`: the `crates/toolu` key-file row names doctor, config, status and setup.

## Open Questions

None blocking. The brainstorm decided the defaults with Jev:

- `config set` writes the user file by default.
- Templates are embedded, with an override.
- The Codex inventory comes from the CLI.
- `status` covers the gate and the repository.

Spec review (Jev `outside_behavior` = `no_project_missing`, 0.96) fixed three contracts the draft left contradictory:

- Outside a git toplevel, repository fields are empty. Gate state is read only when `project_root` exists; push review and waivers need a branch slug.
- AC-6 forbids secret values, not key names. `redact_json` keeps `statusToken` and replaces the value. The doctor summary may contain the path `epic.statusToken`.
- Plugin versions live in `details.plugins`. OpenCode skew is `ok` / `not applicable`. Push-review `document` keeps the v2 field names. `.sh` modules are not warnings.

Residual risk: installed 7.11.0 plugins have no `hookProtocol`, so `doctor` fails `skew` on today's machines until the plugins are updated. That is the #411 rule, not a defect.

## Spec review

**Status:** Approved

First pass (2026-10-07) was Needs changes: the outside-repo sentences disagreed, AC-6 could not be met together with `redact_json` and the epic path error, and `details.plugins` was unspecified. Those are the contracts above. Second pass: every authored section is buildable, each AC has a real input and a runnable check, and no open question changes scope.
