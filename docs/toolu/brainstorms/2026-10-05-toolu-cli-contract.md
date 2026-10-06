# Brainstorm — the `toolu` CLI contract (#442)

Delivery mode, Full path: this is a public interface that hooks, skills and
later sub-issues of epic #402 depend on, and its compatibility rule is hard to
reverse.

## Capsule

- **Outcome:** one `toolu` binary whose clap command tree has a namespace for
  every retained plugin (12 crates: `toolu` hub, the four rule crates through
  it, seven leaf crates), the `commands` and `plugins` built-ins, global
  `--json`/`--host`/`--config-dir`/`--quiet`, the documented exit codes, a
  `toolu hook` fast path that never builds the tree, `toolu commands --json`
  with a JSON Schema, generated `docs/cli/`, and CI checks for docs staleness,
  command-tree compatibility and `toolu --version` startup.
- **Material defaults / non-goals:** no namespace is ported here. Unported
  namespaces get one placeholder verb, `planned`. #412's hook runner and
  launcher grammar stay as they are; only the argv front end changes. Host
  detection (#414), config loading and every real verb stay with their issues.
- **Repository evidence:** PR #476 (#412, about to merge) adds `crates/cli`
  with a hand-written parser and states "#442 replaces the hand-written argv
  parser with clap". `docs/resource-budgets.md` (#410) assigns #442 the
  `toolu --version` row: ≤ 4 ms wall p50, 30 runs after 3 warm-up, through
  `cargo xtask measure`. `rules.json` already names `toolu-cli`'s `output`
  module as a stdio owner. `cargo xtask check-gate-change` already compares
  against the merge base and takes `--title`.
- **Risk:** large mechanical surface (13 crates touched) against jscpd and
  co-located-test rules. #476 conflicts until it merges. Startup wall time on
  shared CI runners has 4× headroom over the expected release-binary cost.
- **Handoff:** spec.

## Axes

### Interface

- **Command tree.** The issue's tree is the starting point; each port finalises
  its namespace. Default: register every namespace now; give unported ones a
  single `planned` verb that reports the planned verbs and the porting issues
  (stdout text, or one JSON document under `--json`). Jev: `one_planned_verb`
  0.94, against registering every planned verb as a stub (0.00) or verb-less
  namespaces (0.05). Stubs would lock unported verbs into the compatibility
  guarantee before their port decides them.
- **Hook grammar.** #412's generator already writes `toolu hook <name> --event E
  --plugin-root DIR` (toolu plugin) and `toolu <plugin-dir> hook <name> …`
  (others) into `hooks.json`, plus a `--hook-protocol` probe per hook spawn.
  Default: keep exactly those forms on a fast path before clap; mirror them in
  the clap tree (`hook` top-level for the toolu plugin, a hidden `hook` verb on
  every other plugin namespace, and the plugin directory name as an alias where
  the namespace differs: `review`/`toolu-review`, `babysit`/`pr-babysit`,
  `epic`/`epic-orchestrator`), so `--help`, `commands --json` and the
  compatibility check see them.
- **Plugin crate API.** Each crate exports `PLUGIN`, `command() -> clap::Command`
  and `run(&ArgMatches, &Ctx) -> Outcome`. `Outcome` (exit, stdout, stderr)
  replaces `ExitCode` because plugin crates may not touch the standard streams
  (rule 14: only `toolu-protocol`, the `toolu-cli` `output` module and xtask);
  `crates/cli` prints it. `Exit` is a typed enum convertible to the process
  exit code. The hub (`crates/toolu`) has one module per toolu namespace, each
  with the same pair, and re-exports the rule crates.
- **`--json` everywhere.** With `--json`, stdout is exactly one JSON document on
  every exit path: data, help, version, a usage error, or a synthesized error
  envelope when a verb failed without JSON. Hooks are excluded: they speak the
  host protocol.

### Data and state

- **Compatibility.** Jev: `merge_base_hookprotocol` 0.98. An xtask step
  compares `docs/cli/commands.json` at the merge base with the working tree;
  a documented command path or flag (placeholders excepted) may disappear only
  when the tree's `hookProtocol` increased (#411: bumped when a documented verb
  breaks) and, with `--title`, the title carries the `!` breaking marker.
  Rejected: a per-major baseline file (release-please bumps the version only
  after the breaking PR merges, and every major needs a manual file) and the
  insta snapshot alone (no automated superset check).
- **The tree carries no version.** A version field would make
  `docs/cli/commands.json` and the snapshot stale on every release bump.

### Failure behaviour

- Usage errors exit 64 with clap's message (and its "a similar subcommand
  exists" tip) on stderr; with `--json` also a JSON error document on stdout.
  `--help`/`--version` exit 0 on stdout. A missing verb prints help on stderr
  and exits 64.
- `toolu hook` keeps #412's semantics: enforcing events exit 2 when the hook is
  absent or blocked.

### Integration

- **Docs.** Jev: `marker_owned` 0.96. `cargo xtask docs-cli` writes
  `docs/cli/README.md`, one page per top-level command (each command's real
  `--help` output), `commands.json` and `commands.schema.json`; every generated
  Markdown file starts with a marker, and `--check` (a gate step) regenerates,
  compares and flags marker files that are no longer produced. `docs/cli.md`
  (the Node installer guide) moves to `docs/cli/installer.md`, unmarked and
  hand-written until #438 ports the installer; references follow it, and the
  OpenCode surface that copies linked docs is regenerated.
- **`toolu plugins`.** Jev was split (`crate_now` 0.50, `cli_builtin` 0.45).
  Decision: a `crates/cli` built-in placeholder (like `commands`) until #438
  creates `crates/plugins` with real code. It keeps the inventory exact (the
  12 manifests ↔ 12 plugin crates ↔ plugin owners) without a special case for
  a crate that has no manifest.
- **Inventory.** A `crates/cli` integration test reads `plugins/*/.claude-plugin`,
  the `crates/` directories and the registry owners exported by
  `toolu commands --json`, and fails when any side lacks a name.

### Constraints

- **Dependencies.** clap's builder API with `default-features = false` (std,
  help, usage, error-context, suggestions): no derive (its syn 2 would
  duplicate xtask's syn 3 under `multiple-versions = "deny"`), no colour
  crates. Dev: insta (snapshot), assert_cmd (black-box), jsonschema
  (`default-features = false`). A probe workspace with the repo's `deny.toml`
  passed `cargo deny check` (advisories, bans, licences, sources); cold build
  of the dev set took 29 s.
- **jscpd.** Placeholder crates stay a few lines each and delegate to a shared
  `Planned` descriptor in `toolu-runtime`, so 13 near-identical crates make no
  10-line clone.
- **Startup.** Jev: `xtask_release_ci` 0.92. `cargo xtask check-startup --bin
  <toolu>` spawns `toolu --version` 3 + 30 times and compares the p50 wall
  with `benchmarks/startup-budgets.json`; the Linux `rust` CI job runs it on
  the release binary. Rejected: extending the TypeScript hook bench's strict
  hook-budget schema, and a `cargo test` measurement of the debug,
  coverage-instrumented binary.

## Rejected alternatives

- Registering every planned verb now as a 69-exit stub.
- A hand-maintained per-major command-tree baseline.
- Folding the Node installer guide into `toolu plugins` help text (it would
  document a different program).
- clap derive (duplicate `syn` major versions).
- Plugin crates printing for themselves (breaks the stdio capability rule).

## Jev record

`ask` over the five forks above (state: issue excerpts and repository facts).
The plain `ask` output failed the wrapper's parse ("expected a typed answer for
every question") while `--raw` returned all five answers from `jev-1.13.0`;
the raw answers are the ones recorded here.
