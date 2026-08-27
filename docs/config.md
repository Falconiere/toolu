# Toolu Config

Runtime opt-out for individual toolu components. No config file is
required — every component defaults to enabled. Drop in a file only to
disable what you do not want.

## Locations

- Claude Code: `~/.claude/toolu.config.json` and
  `<repo>/.claude/toolu.config.json` (`CLAUDE_CONFIG_DIR` remains supported).
- Codex: `${CODEX_HOME:-~/.codex}/toolu.config.json` and
  `<repo>/.codex/toolu.config.json`.
- `TOOLU_CONFIG_DIR`, `TOOLU_PROJECT_DIR`, and
  `TOOLU_PROJECT_CONFIG_DIRNAME` are explicit cross-host overrides.

Both are optional. When both exist they are deep-merged via `jq '. * .'`;
project values win on conflict. Missing keys default to **enabled**.

Requires `jq` (already a hard dependency of every toolu hook). With `jq`
absent or the JSON malformed, the loader warns once on stderr and falls
back to "all enabled".

## Schema

```json
{
  "version": 1,
  "skills":  { "<name>": true | false },
  "hooks":   { "<name>": true | false },
  "mcp":     { "<server>": true | false },
  "models":  { "enabled": true | false,
               "<class>": "haiku|sonnet|opus|fable|inherit",
               "codex": { "<class>": { "model": "<slug>",
                                         "reasoningEffort": "low|medium|high|xhigh|max|ultra" } } },
  "lang":    { "ts":   { "maxFileLines": 300, "maxFnLines": 60, "noMocks": true },
               "rust": { "maxFileLines": 500, "maxFnLines": 50, "maxImplLines": 200, "noMocks": true } },
  "docsSync": { "mode": "advise|block|off",
               "surfaces": ["README.md", "docs/*.md", "*/SKILL.md", "AGENTS.md", "CLAUDE.md"],
               "surfaceExcludes": ["docs/releases/*"],
               "codeSurfaces": ["*.ts", "*.rs", "*.sh", "*plugin.json"] },
  "telemetry":  { "enabled": true },
  "agentTier":  { "mode": "advise|block|off" },
  "planLedger": { "blockOnUncoveredAcs": false },
  "gates":      { "preset": "strict|balanced|relaxed",
                  "<gate>": { "mode": "block|ask|advise|off" },
                  "sweep": true,
                  "stateTtlHours": 24,
                  "telemetryRetentionDays": 7 },
  "permissions": { "autoAllow": true,
                   "allow": ["Bash(*)", "Edit", "Write"] }
}
```

`version` is reserved for future schema bumps; v1 is the current value.
See `plugins/toolu/settings/toolu.config.example.json` for a fully-populated example.

### Gate modes (`gates`)

How firmly a gate says no is configuration, not a constant. Full reference:
`plugins/toolu/hooks/docs/gates.md`.

| Mode | Effect |
|------|--------|
| `block` | `permissionDecision: "deny"` — the call does not run |
| `ask` | `permissionDecision: "ask"` — the user decides |
| `advise` | `additionalContext` only — nothing is stopped |
| `off` | no output at all |

`ask` requires a host that can prompt; on Codex it degrades to `advise`, so the
gate still speaks rather than silently doing nothing.

`gates.preset` sets all seven at once and **defaults to `balanced`**:

| Gate | `strict` | **`balanced`** | `relaxed` |
|------|----------|----------------|-----------|
| `pushReview` | block | **ask** | advise |
| `qualityGate` | block | **block** | advise |
| `commitGate` | block | **advise** | advise |
| `bashCommands` | block | **ask** | ask |
| `planLedger` | block | **advise** | off |
| `docsSync` | block | **advise** | off |
| `agentTier` | block | **advise** | off |

`protected-files` and `mcp-blocker` are deliberately absent: they deny at every
preset. Presets relax judgement calls, not guardrails.

Precedence is `gates.<name>.mode` → the legacy top-level key (`docsSync.mode`,
`agentTier.mode` only) → the preset table → `balanced`. An unrecognized value
warns once on stderr and falls through, so a typo never silently opens a gate.

Two consequences worth stating plainly:

- **`gates.bashCommands: "off"` is a security decision, not a convenience
  one.** It disables the whole `settings/bash-denylist.txt` check, including
  the `node -e` / `bun -e` arbitrary-code-execution rules. `ask` (the default)
  keeps the question; `off` removes it.
- **`permissions.deny` in Claude Code settings overrides a hook's `ask`.** A
  command denied there is denied outright and toolu never gets to prompt.
  `permissions.allow` does *not* have that power — an allowlisted command still
  reaches these gates. This is why toolu's shipped
  `settings/permissions.fragment.json` no longer denies `git push --force` or
  `node -e`: those are gate decisions now, and a deny rule would silently
  outrank the mode you configured.

`qualityGate` in `block` mode stops `git commit` and `git push` and nothing
else — reading, searching, editing, and running commands stay open while the
gate is red. `MY_CLAUDE_QUALITY=off` remains a full kill switch.

#### Push waivers

With `pushReview` in `ask` mode, a "yes" is remembered for that exact diff:
the gate writes a pending marker, `post-tools/modules/push-waiver.sh` promotes
it once the push actually succeeds, and the next push of the same code is
silent. A new commit changes the diff SHA and the gate asks again. A refused
prompt records nothing.

#### State sweeping (`gates.sweep`, `gates.stateTtlHours`, `gates.telemetryRetentionDays`)

`SessionStart` reclaims spent transient state under `<repo>/<host-dir>/tmp` so
`rm -rf .claude/tmp/*` stops being a manual chore:

| Target | Reclaimed when |
|--------|----------------|
| `push-review/`, `plan-ledger/`, `docs-sync/` (incl. `.waiver.json`, `.pending-waiver.json`) | the branch is gone, merged into base, or the file is older than `stateTtlHours` (default 24) |
| `quality-gate-status.json` | it is passing, or it is failing about files that no longer exist |
| `telemetry/<slug>.jsonl` | lines older than `telemetryRetentionDays` (default 7) are dropped; the file goes only if nothing recent remains |

The **current branch's** state is never reclaimed, however old. A failing gate
about files that still exist survives too — a violation is not fixed by
forgetting it. Anything else under `tmp/` is left alone. `gates.sweep: false`
disables the sweep; every failure is best-effort and can never keep a session
from starting.

### Host permissions (`permissions`)

The first toolu session in a repository writes an allowlist into that repo's
`.claude/settings.local.json` — by default `Bash(*)`, `Edit`, `Write`
(`Bash(*)` subsumes `Bash(git:*)` and `Bash(gh:*)`) — and records a sentinel at
`.claude/tmp/.permissions-written` so it never writes again. Delete a rule by
hand and it stays deleted.

- `permissions.autoAllow: false` disables the write entirely.
- `permissions.allow` replaces the default set with your own.
- Existing entries and any `deny` block are preserved: the merge is a union,
  and a `settings.local.json` that cannot be parsed is left byte-identical.
- The sentinel deliberately does **not** live in `toolu.config.json`, which is
  committed — a marker there would ride into the repo and every clone would
  think the write had already happened.
- Claude Code only; on Codex this is a no-op.

**This is a broad grant.** `Bash(*)` means the host stops prompting for shell
commands, destructive ones included. toolu's own gates and the `deny` rules in
your settings still apply; nothing else does. Set `autoAllow: false` if that
is not the trade you want.

### Quality thresholds (`lang`)

The rust-quality and ts-quality gates' line limits are not hardcoded. Each threshold resolves
with this precedence (first hit wins, always a positive integer):

1. **Project / user override** — the `lang.<ts|rust>.<key>` value above.
2. **Native linter config** (TS `maxFileLines` only) — the `max-lines` rule from
   the *active* linter's JSON config: `.oxlintrc.json` when oxc is detected,
   `.eslintrc.json` when eslint is (detection precedence is biome > oxc > eslint;
   biome has no `max-lines`, so it falls through to the default). Only the active
   linter's file is read — a repo carrying both (e.g. mid-migration) does not
   chain between them. All encodings parse: `N`, `["error", N]`,
   `["error", {"max": N}]`. Flat config `eslint.config.{js,mjs,ts}` is JavaScript
   and not parsed — it falls through.
3. **Built-in default** — TS `maxFileLines` 300 / `maxFnLines` 60; Rust
   `maxFileLines` 500 / `maxFnLines` 50 / `maxImplLines` 200.

The file-size limit counts real code only: `count_code_lines`
(`plugins/toolu/hooks/lib/detect.sh`) excludes blank lines and `//` + `/* */`
comments. It is a lexical heuristic, not a parser — a `//` inside a string literal
(e.g. a `"https://…"` URL) is treated as a line-ending comment, so a file dense in
such literals can count slightly low. The gate deliberately fails *toward*
flagging: when the scan ends mid-`/* */` (an unterminated block, or a `/*` inside a
string), it falls back to the raw line count rather than risk under-counting an
oversized file.

A value of `0`, a negative, or `"off"` is treated as "no override" and falls
through to the next layer — it does not mean a limit of zero. A stringified
positive integer (`"maxFileLines": "120"`) is accepted and coerced to a number,
so configs copy-pasted from sources that quote numbers still work. The gate never
invokes biome/oxc/eslint/prettier/clippy/rustfmt; detecting them only tunes
advisory wording. Resolver: `plugins/toolu/hooks/lib/quality-config.sh`.

### No-mock test gate (`lang.<ts|rust>.noMocks`)

`lang.ts.noMocks` / `lang.rust.noMocks` (default `true` — blocking) control the
mechanical no-mock-test concerns: `ts-quality/hooks/concerns/85-no-mocks.sh`
(TS/TSX test files — `jest.mock`/`vi.mock`/`jest.fn`/`vi.fn`/`sinon.*` calls and
`ts-mockito` imports) and `rust-quality/hooks/concerns/70-no-mocks.sh` (`src/`
mock *definitions* — `#[automock]`, `#[cfg_attr(..., automock)]`, `mock! {...}`
— and `tests/`/`*_test.rs`/`*_tests.rs` mock *imports* — `mockall::`/`faux::`).
Both reuse the `ast-grep scan --inline-rules` pattern from
`60-error-handling.sh`; a real ast-grep failure (non-zero exit or unparseable
JSON) is reported as a gate error, never a silent pass. Set to `false` to
opt out — read via the boolean `quality_flag` reader in
`plugins/toolu/hooks/lib/quality-config.sh` (`_qc_project_override` cannot
carry a boolean, hence the separate reader).

### Model routing (`models`)

Which model tier a delegated task is handed to, keyed by the **class of work**.
`SessionStart` injects the resolved table into every session, so the agent routes
subagents by task complexity without being asked; the full rubric lives in
`plugins/toolu/skills/orchestrator/references/model-routing.md`.

| Class | Claude default | Codex default | Work that belongs here |
|-------|----------------|---------------|------------------------|
| `mechanical` | `haiku` | Luna / medium | Listings, single-symbol lookups, literal search, renames, formatting |
| `exploration` | `sonnet` | Terra / medium | Read-only search across many files |
| `implementation` | `sonnet` | Terra / medium | A bounded, already-decided edit plus its tests |
| `review` | `sonnet` | Terra / high | Diff review, audits |
| `synthesis` | `opus` | Sol / high | Reconciling several agents' findings into one answer |
| `architecture` | `opus` | Sol / high | Design, trade-offs, hard-to-reverse decisions |

Values are **aliases, not version ids** — `sonnet` keeps meaning "the current mid
tier" across model releases, so a config written today survives the next model
generation. Accepted: `haiku`, `sonnet`, `opus`, `fable`, `inherit` (`inherit` =
the lead thread's model). Anything else is rejected with a stderr warning and
falls back to the built-in default, so a typo mis-tiers nothing.

Those legacy `models.<class>` aliases remain Claude-only and backward
compatible. Codex reads `models.codex.<class>.model` and
`models.codex.<class>.reasoningEffort`; model must be a non-empty slug and an
invalid effort warns and falls back to the class default.

`models.enabled = false` suppresses the SessionStart injection; the tiers still
resolve for anything that reads them (`toolu_model` in
`plugins/toolu/hooks/lib/config.sh`).

Config remaps the **rubric** and the tiers passed on delegated work. It does not
rewrite installed agent definitions. Claude reads the plugin's agent
frontmatter directly. Codex profiles live under `${CODEX_HOME:-~/.codex}/agents`
and are managed explicitly with `$toolu:setup`; customize a copied profile if
you need a fixed tier that differs from the routing table.

Plan steps can pin their own tier: a step in a `## Steps (machine-readable)`
block may carry `"model": "<alias>"`, validated at parse time and surfaced by
`plan-ledger.sh status` as `model=<alias>` on the summary line for the next step.

### Docs-sync surfaces (`docsSync`)

The docs-sync backstop (`plugins/toolu/hooks/pre-tools/modules/docs-sync.sh`)
fires on `git push` when the branch diff changes code but no documentation
surface — a nudge to keep user-facing docs in sync with behavior. It is
silenced by a diff-`sha`-keyed attestation the agent writes to the host-native
project state path: `.claude/tmp/docs-sync/<branch-slug>.json` for Claude or
`.codex/tmp/docs-sync/<branch-slug>.json` for Codex.

`docsSync.mode` (via `toolu_string`, unrecognized values warn and fall back)
controls enforcement:

| Mode | Behavior |
|------|----------|
| `advise` (default) | Emits `additionalContext` only — never blocks the push. |
| `block` | Denies the push when code changed with no doc surface and no matching attestation; a valid attestation still allows it. |
| `off` | Fully disabled — no telemetry (`docs_nudge`/`docs_attested`), no output; indistinguishable from the module not existing. |

Unlike `agentTier.mode=off` above (which keeps recording its `delegation`
telemetry), `docsSync.mode=off` is fully silent — the advisory nudge is this
module's only artifact, so turning it off turns off everything.

Three glob sets tune the code/doc classification; each resolves *project/user
override → built-in default* (resolver
`plugins/toolu/hooks/lib/docs-sync-config.sh`):

| Key | Default | Meaning |
|-----|---------|---------|
| `docsSync.surfaces` | `README.md`, `*/README.md`, `docs/*.md`, `*/SKILL.md`, `AGENTS.md`, `*/AGENTS.md`, `CLAUDE.md`, `*/CLAUDE.md` | doc files whose change **satisfies** the check |
| `docsSync.surfaceExcludes` | `docs/releases/*`, `*/docs/releases/*` | doc paths carved back out (release notes are per-release, not per-task) |
| `docsSync.codeSurfaces` | `*.ts`, `*.rs`, `*.sh`, `*/commands/*`, `*plugin.json`, `*.config.json` | code files whose change **demands** a doc touch |

Globs are matched with bash `case` fnmatch where a single `*` **crosses `/`** —
so `docs/*.md` already covers nested paths (which is *why* `surfaceExcludes`
exists: without it, `docs/*.md` would swallow `docs/releases/*.md`). A diff path
counts as a doc touch when it matches `surfaces` **and not** `surfaceExcludes`.
Setting any key replaces (does not merge with) that list's default.

### Workflow telemetry (`telemetry`)

`telemetry.enabled` (default `true`) gates `plugins/toolu/hooks/lib/telemetry.sh`,
the one append path every gate/lib event funnels through. When enabled it writes
one JSONL line per event to the host-native
`.claude/tmp/telemetry/<branch_slug>.jsonl` or
`.codex/tmp/telemetry/<branch_slug>.jsonl`
(`TELEMETRY_DIR` overrides the directory in tests): `step_run` (plan-ledger
step executions), `gate_fail`/`gate_clear` (quality-gate transitions),
`push_check` (push-review decisions), `docs_nudge`/`docs_attested` (docs-sync),
`delegation` (Claude `Agent`/`Task` or Codex `spawn_agent` model-tier calls), and `ac_coverage` (spec AC
coverage counts on each push check). Set `telemetry.enabled: false` to disable
all of it — every write site still succeeds (a telemetry bug never fails the
caller's real work), it just writes nothing.

### Agent-tier advisory (`agentTier`)

`agentTier.mode` (default `advise`) controls `plugins/toolu/hooks/pre-tools/agent-tier.sh`,
a standalone `PreToolUse` hook on Claude `Agent`/`Task` and Codex `spawn_agent`
calls. It always records a
`delegation` telemetry event (model, subagent_type, and — when a plan ledger
exists for the branch — the running/next step's id and declared `model`). When
a delegation's `model` differs from that step's declared (non-null) `model`, it
nudges: `advise` emits `additionalContext`, `block` denies the call,
`off` records telemetry only. A delegation with no `model` param (tier-inherit)
is always legitimate and never nudged.

`agentTier.mode=off` is a deliberate, narrower "off" than `docsSync.mode=off`
below: it silences only the advisory — the `delegation` telemetry event still
fires on every call, because that telemetry *is* the primary artifact this
feature produces (model-routing analytics), not a side effect of the nudge.

### AC-coverage promotion (`planLedger`)

`planLedger.blockOnUncoveredAcs` (default `false`) promotes spec AC coverage
from advisory to blocking in `plugins/toolu/hooks/pre-tools/modules/plan-ledger.sh`.
Either way, every push check appends an `ac_coverage` telemetry event with
`covered`/`uncovered` counts (reusing `pl_ac_coverage_lines`). With the default
`false`, an uncovered AC only shows up in that count and in `plan-ledger.sh
status`'s AC-coverage report. With `true`, `git push` is denied naming the
uncovered spec `AC-<n>` id(s) until a fresh-green step's `ac_refs` covers them.

### Recognized names

| Category | Names                                                                              |
|----------|------------------------------------------------------------------------------------|
| `skills` | `comemory`, `ast-grep` (the only skill keys any hook reads)                        |
| `hooks`  | `session-start`, `user-prompt-submit`, `pre-tools`, `post-tools`, `pre-compact`, `session-end` |
| `mcp`    | any MCP server name — e.g. `canva`, `figma`                                        |
| `models` | `enabled`, the six Claude class aliases, and `codex.<class>.{model,reasoningEffort}` |

Unknown names are silently ignored (forward compatible).

## Effects

- `skills.<name> = false`
  - The hooks that reference the skill behave as if its CLI is not
    installed AND they suppress the "not installed" warning. Skill files
    themselves stay on disk.
  - Concretely: `skills.comemory = false` silences the `MANDATORY: recall`
    hint in `UserPromptSubmit`, the comemory entry in the `SessionStart`
    "missing tools" warning, and the comemory reminder in `PreCompact` and
    `SessionEnd`. `skills.ast-grep = false` removes the ast-grep STOP /
    install-hint advisories in `search-nudge` (a registry module shipped
    by the ast-grep plugin); the generic `grep/rg → Grep tool` advisory
    still fires.

- `hooks.<name> = false`
  - The named hook exits early and emits nothing. Its stdin is drained
    first so the host's hook IPC does not stall.
  - **Exception — `session-end` reminder is opt-IN**: the end-of-session
    comemory "save your learnings" reminder is OFF by default (the agent-memory
    protocol already saves proactively, so the Stop-time nag is redundant
    noise). It emits only when you set `hooks.session-end: true`. Every other
    hook is opt-out (on unless set to `false`).
  - **`session-end` also drives autonomous comemory maintenance** (a once-per-day
    `mine`/`prune`/`gc` pass, local and token-free) — this is opt-OUT, ON by
    default, independent of the opt-IN reminder. Setting `hooks.session-end: false`
    disables BOTH the reminder and the maintenance, keeping the "exits early,
    mutates nothing" contract. (`skills.comemory: false` also disables it.)

- `mcp.<name> = false`
  - Any `mcp__<name>__*` tool invocation is blocked at `PreToolUse` with
    `permissionDecision: "deny"`. The deny reason names the source
    (`see plugins/toolu/settings/mcp-blocklist.txt` for file entries, or
    `disabled via toolu config (mcp.<name>=false …)` for config
    entries) so users know where to undo it.
  - A `mcp-blocklist.txt` line may add an optional `-> <text>` redirect hint
    after the server prefix; when that server is denied the text is appended to
    the deny reason (e.g. to steer the user to a replacement skill). See
    `plugins/toolu/settings/README.md`.
  - Matcher wiring lives in `plugins/toolu/hooks/hooks.json`, which routes
    the `mcp__` prefix through `plugins/toolu/hooks/pre-tools/modules/mcp-blocker.sh`.

Skills, hooks, Claude agents, and Claude commands are loaded from plugin
manifests at session start, so config does not remove those files from host
discovery. Codex command equivalents are ordinary plugin skills. Codex custom
agents are intentionally separate: `$toolu:setup` previews and installs the
five bundled TOML profiles under `${CODEX_HOME:-~/.codex}/agents`, refusing
unmanaged conflicts and preserving timestamped backups.

## comemory version

toolu targets **comemory ≥ 0.8.0** (pinned as `COMEMORY_MIN_VERSION` in
`plugins/toolu/hooks/lib/detect.sh`). The wrapper uses comemory's full verb
surface — the retrieval-quality loop (`feedback`/`mine`/`tune`/`eval`/`prune`/
`gc`/`rebuild`) and **comemory** (`search-code`/`index-code`/`graph`). An older
binary lacks some of these and will error on them, so session start emits a
non-fatal upgrade WARN when it detects one. Basics (`search`/`save`/`list`)
still work. Upgrade with `brew upgrade Falconiere/tap/comemory` (comemory is not published to crates.io; the Homebrew tap or the curl installer are the canonical paths). Run `/comemory:setup` on Claude or `$comemory:setup` on Codex to verify and wire it.

## Examples

Disable comemory completely (no recall hint, no install nag):

```json
{ "version": 1, "skills": { "comemory": false } }
```

Disable a single hook only in this project:

```json
{ "version": 1, "hooks": { "user-prompt-submit": false } }
```

Block several MCP servers without touching the blocklist file:

```json
{ "version": 1, "mcp": { "canva": false, "figma": false } }
```

User-global disables, project-local re-enable:

```jsonc
// Claude: ~/.claude/toolu.config.json
// Codex:  ~/.codex/toolu.config.json
{ "version": 1, "skills": { "ast-grep": false } }

// Claude: <repo>/.claude/toolu.config.json
// Codex:  <repo>/.codex/toolu.config.json
{ "version": 1, "skills": { "ast-grep": true } }
```

## Shrink session footprint

Every enabled plugin's skill `description` fields and the Session Protocol load
into the model's context at session start — recurring cost that shrinks your
usable context window. To reclaim it:

- **Disable a whole plugin you don't use** — via Claude Code's `/plugin` UI or
  Codex's `codex plugin remove <name>@toolu`. This is what actually drops its skill
  descriptions from session load. Note the toolu `skills.<name>` config
  does **not** do this: it only gates hook behavior for `comemory`/`ast-grep`
  (see *Effects* above — `SKILL.md` files stay on disk and their descriptions
  still load).
- **MCP tools defer natively** — supported hosts load their schemas on demand,
  so an idle connector costs little. Use `mcp.<server>: false` to
  block a noisy one entirely.

The harness caps its *own* injected footprint with
`plugins/toolu/scripts/context-budget.sh` (run in CI): word ceilings on the
Session Protocol + per-language docs and on every skill `description`, so the
baseline cannot silently regrow.
