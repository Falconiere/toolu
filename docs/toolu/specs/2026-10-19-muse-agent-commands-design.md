# Muse agent commands — Design

**Date:** 2026-10-19  **Status:** Approved  **Author:** Claude  **Topic:** Falconiere/toolu#548

## Problem

Muse rejects the `agents` capability, so the five toolu agents
(`plugins/toolu/agents/{architect,deep-explore,implementer,quick-task,research-agent}.md`)
cannot ship to Muse as agents. Skills such as `orchestrator` point at them, so a
Muse session loses the tier-pinned roles unless they are mapped onto a capability
Muse validates. The mapping must keep one source text per agent, and the gate
must notice when the Muse view drifts from it.

## Non-Goals

1. The existing commands (`commit`, `doctor`, `review-and-commit`, `status`)
   and the generated hooks in the toolu Muse manifest belong to #547, as do the
   manifests of the other 11 plugins. The six toolu skills are included here
   only so a skill that references an agent can be shown resolving on Muse.
   The gate does not judge skill entries.
2. The Codex agent profiles (`plugins/toolu/assets/agents/*.toml`) do not
   change.
3. No tier pin on Muse. Muse has no custom agent definitions, and the
   frontmatter `model` stays a Claude alias.
4. Running `muse` in CI. Live validation is evidence recorded in this PR, and
   #552 owns Muse CI registration.

## Architecture

**Mapping rule (uniform): every `agents/<name>.md` is a Muse command with id
`<name>` whose `path` is the agent file itself.** There is no generated or copied
Muse view. The manifest entry points at the Claude/Codex source, so an edit to
an agent's text needs no Muse update.

Decisive evidence, from a scratch bundle on Muse Code 1.4.11 in an isolated
`XDG_DATA_HOME`:

- `muse plugins validate` accepts `{"id":"architect","path":"agents/architect.md"}`
  for all five agents with `valid: true` and no diagnostics. The Claude
  frontmatter keys `tools` and `model` are tolerated.
- The TUI slash menu lists `/architect`, `/deep-explore`, `/implementer`,
  `/quick-task` and `/research-agent` with each agent's frontmatter
  `description`, including the folded `>-` one.
- Submitting `/quick-task list files` sent the agent body (frontmatter
  stripped) followed by `list files`. The prompt is the agent text.
- The qualified `/toolu:quick-task find X` resolves the same way. Muse also
  qualifies names on a collision: the toolu skill `deep-research` is listed as
  `/toolu:deep-research` beside the built-in `/deep-research`. **The Muse
  invocation the docs give is therefore `/toolu:<agent>`.** It always resolves,
  and a bare `/<agent>` works while no other command takes the name.
- All six `plugins/toolu/skills/*` pass `muse skills validate` with no
  diagnostics. A bundle that holds them beside the five agent commands
  validates clean, and they appear as `/orchestrator`, `/debug` and so on.

Rejected alternatives:

- **(b) skills.** A skill entry pointing at `agents/<name>.md` passes plugin
  validation, but `muse skills validate` needs a directory holding `SKILL.md`,
  and #547 requires that check for every referenced skill. Folding the agents
  into skills would copy or move their text.
- **(c) reminders.** A reminder runs as a reminder agent with only
  `submit_reminder_decision`, behind a decision envelope. The agents are
  delegated task roles, not standing policies.

Jev compared the three mappings (`commands` 1.0) and the gate placement (new packaging module 0.47, ahead of
`packaging_assets.rs` at 0.37). The module is new because `packaging_assets.rs`
is already at 307 lines against the 300-line file limit. Jev first picked an
agents-only manifest (0.99). Spec review then found that no skill could show an
agent reference resolving on Muse, and the re-asked choice moved to agents plus
skills (0.97).

On Muse a command applies the role in the current thread. It does not spawn a
tier-pinned subagent. The skills that list the agents say this next to each
host's invocation syntax.

Reuse:

- `packaging::package` gains one call.
- `packaging_assets::{dirs, read_json}` lists plugins and reads JSON.
- `packaging_catalog::release_tracks` checks release-please.

## Interfaces / Schema

`plugins/toolu/.muse-plugin/plugin.json`:

```json
{
  "schemaVersion": 1,
  "name": "toolu",
  "displayName": "toolu",
  "version": "<workspace version>",
  "description": "<the .claude-plugin description>",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "skills": [
      { "id": "commit", "path": "skills/commit/SKILL.md" },
      { "id": "debug", "path": "skills/debug/SKILL.md" },
      { "id": "deep-research", "path": "skills/deep-research/SKILL.md" },
      { "id": "orchestrator", "path": "skills/orchestrator/SKILL.md" },
      { "id": "review-and-commit", "path": "skills/review-and-commit/SKILL.md" },
      { "id": "setup", "path": "skills/setup/SKILL.md" }
    ],
    "commands": [
      { "id": "architect", "path": "agents/architect.md" },
      { "id": "deep-explore", "path": "agents/deep-explore.md" },
      { "id": "implementer", "path": "agents/implementer.md" },
      { "id": "quick-task", "path": "agents/quick-task.md" },
      { "id": "research-agent", "path": "agents/research-agent.md" }
    ],
    "hooks": [],
    "mcpServers": [],
    "reminders": []
  }
}
```

`crates/xtask/src/packaging_muse.rs`:

```rust
/// Muse bundles: lockstep version, release tracking, and one command per agent.
pub(crate) fn check_muse(root: &Path, version: &str, release: &Value) -> Result<usize, String>
```

It returns the number of agent commands it checked. `packaging::package` calls
it and the packaging line reports it as `{n} Muse agent commands`.

For each `plugins/<p>` it checks:

- the agents: the sorted stems of `agents/*.md`;
- the manifest, `plugins/<p>/.muse-plugin/plugin.json`, when present: its
  `version` equals `package.json`, release-please tracks it at `$.version`,
  its top-level keys are within `schemaVersion`, `name`, `displayName`,
  `version`, `description`, `compat` and `capabilities` (Muse warns
  `unsupported-field` on any other key, which breaks the zero-diagnostics bar),
  `capabilities` has no `agents` key, and `capabilities.commands`, when
  present, is an array;
- every agent `a`: there is a command entry with `id == a` and
  `path == "agents/<a>.md"`;
- every command entry whose `path` starts with `agents/`: the path names an
  existing agent and its id equals that agent's stem.

Configuration:

- `release-please-config.json` gains the extra-file
  `plugins/toolu/.muse-plugin/plugin.json` at `$.version`.
- `.github/ci-paths.json`: the `rust` group gains
  `plugins/*/.muse-plugin/plugin.json` and `plugins/*/agents/*.md`, so an agent
  edit runs the gate. `releaseOnly.paths` gains
  `plugins/*/.muse-plugin/plugin.json`, so a release bump stays release-only.

## Failure modes and edge cases

Each failure is an `Err` from `check_muse`. The packaging step prints it as
`validate-plugin-packaging: <message>` and reports Findings, so the gate fails.
Every message names the manifest path:

- **Agent without a command** (an agent added or renamed, or an entry
  removed): `<manifest>: agents/<a>.md has no Muse command {"id":"<a>","path":"agents/<a>.md"}`.
- **Stale entry** (it points at `agents/x.md` and no such agent exists):
  `<manifest>: command <id> points at agents/x.md, which is not an agent source`.
- **Id differs from the stem:**
  `<manifest>: command <id> must be named <stem> after agents/<stem>.md`.
- **An unknown top-level key** (`agents`, `hookProtocol`, `author`, …):
  `<manifest>: top-level <key> is not a Muse manifest field; Muse warns unsupported-field`.
- **An `agents` key under `capabilities`:**
  `<manifest>: Muse rejects the agents capability; map each agent to a command`.
- **A plugin with agents and no Muse manifest:**
  `plugins/<p> has agents/ but no .muse-plugin/plugin.json`.
- **Version or release drift:** `<manifest> version differs from package.json`,
  or `release-please is missing <manifest>`.
- **Invalid JSON, or `commands` that is not an array:** `invalid JSON: <manifest>: …`,
  or `<manifest>: capabilities.commands is not an array`.

These cases pass:

- A plugin with no Muse manifest and no agents (#547 adds the rest).
- An `agents/` directory holding only `.gitkeep`.
- Command entries that do not point into `agents/`.

## Acceptance criteria

- **AC-1:** Given the committed `plugins/toolu`, `muse plugins validate plugins/toolu --json`
  reports `valid: true`, no diagnostic other than the one `multiple-manifests`
  layout warning (see Deviations), the six skills, and five commands
  whose paths are the five `agents/*.md` files. `muse skills validate` passes
  for each of the six skill directories.
- **AC-2:** With `plugins/toolu` installed in an isolated Muse store, the TUI
  slash menu offers all five commands. Invoking `/toolu:architect <task>` and
  `/toolu:quick-task <task>` submits the body of `agents/architect.md` or
  `agents/quick-task.md`, frontmatter stripped, followed by the task.
- **AC-3:** Given a copy of the real `plugins/toolu` in a temp root,
  `packaging_muse::check_muse` returns `Ok(5)`. Renaming
  `agents/implementer.md`, deleting its command entry, adding a stale
  `agents/` entry, renaming an entry's id, adding `capabilities.agents`,
  adding a top-level `agents` or `author`, deleting the manifest, changing its version, or
  dropping its release-please
  extra-file each returns the matching `Err` from Failure modes.
- **AC-4:** Editing the text of `agents/implementer.md` without touching the
  manifest still passes `check_muse`, because the Muse view is the source.
- **AC-5:** `cargo xtask packaging` and the full `cargo xtask gate` pass on this
  repository. The packaging line reports `5 Muse agent commands`.
- **AC-6:** The toolu README states the mapping rule and the Muse invocation.
  The orchestrator skill and both `model-routing.md` copies give each agent's
  Claude, Codex and Muse syntax. `cargo xtask check-markdown-cli` passes.
- **AC-7:** In the same isolated Muse session, `/orchestrator` submits the
  committed `skills/orchestrator/SKILL.md` body, which names Muse
  `/toolu:quick-task`. That command then resolves as in AC-2, so a skill's
  agent reference resolves on Muse. On Claude the same line names the
  `toolu:quick-task` agent that the Claude plugin ships.

## Acceptance evidence

- **AC-1:** Run `muse plugins validate plugins/toolu --json | jq '{valid,diagnostics,commands:[.capabilities.commands[].path]}'`
  on the real bundle, and `muse skills validate` on each skill directory.
  Record the output in the PR body.
- **AC-2, AC-7:** Run `muse plugins install plugins/toolu` with a scratch
  `XDG_DATA_HOME`, then `muse --provider echo --trust-workspace` in tmux. The
  echo provider returns the submitted prompt, so the pane shows the expansion.
  Capture the pane for the `/toolu:` filter, both agent invocations and
  `/orchestrator`, then remove the plugin from the scratch store. Record it in
  the PR body. A shadowed name would show up here as a missing menu row.
- **AC-3, AC-4:** Unit tests in `crates/xtask/src/tests/packaging_muse_test.rs`
  copy the real repository files (`plugins/toolu/agents/*.md`, the manifest,
  `package.json`, `release-please-config.json`) into a tempdir, then apply each
  mutation. The boundaries are a `.gitkeep`-only `agents/` and a plugin without
  agents or a manifest. Run `cargo test -p xtask packaging_muse`.
- **AC-5:** `cargo test -p xtask packaging` (`this_repository_is_packaged`),
  then `toolu epic job -- cargo xtask gate`.
- **AC-6:** Read the diffs, then run `cargo xtask check-markdown-cli` and
  `cargo xtask gate --only context-budget`.

## Documentation impact

- `plugins/toolu/README.md`: a Muse section with the mapping rule, the
  `/toolu:<agent>` invocation, a note that it runs in the current thread, and the
  validate command.
- `plugins/toolu/skills/orchestrator/SKILL.md` and both identical
  `model-routing.md` copies (`plugins/toolu/skills/orchestrator/references/`,
  `plugins/delivery-flow/skills/delivery-flow/references/`): the Muse syntax for
  each agent.
- `AGENTS.md`: the CI section's sentence that lists the `rust` group gets the
  two new globs, and the release-only sentence names the Muse manifest. The
  `packaging` gate step keeps its name.

## Open Questions

- #547's scenario expects the toolu manifest to list "4 commands". Under this
  mapping it lists 9 (the 4 existing commands plus 5 agent commands). Owner:
  #547. Not blocking. The PR body notes it.
- Muse has a built-in `/doctor` skill. Muse qualified the clashing
  `deep-research` as `/toolu:deep-research`, so a plugin `doctor` command is
  likely reached as `/toolu:doctor`. Owner: #547, when it adds the existing
  commands. Not blocking.

## Deviations

- **The `multiple-manifests` warning.** It was found during execution, and
  this is how the spec reads "zero diagnostics" because of it. Running
  `muse plugins validate plugins/toolu` on the real directory returns `valid:
  true` with one warning: `multiple-manifests` ("selected
  `.muse-plugin/plugin.json`; ignoring `.claude-plugin/plugin.json`,
  `.codex-plugin/plugin.json`"). The isolated scratch bundle never showed it
  because it held no foreign manifests. Any plugin directory that also ships
  the Claude and Codex manifests gets this warning. The epic's distribution
  spike (#541, `docs/research/muse-plugin-distribution.md`) chose that
  colocated layout and accepted the warning. It assigns re-probing to #553 in
  case the warning becomes an error. This PR therefore holds AC-1 to `valid:
  true` and no diagnostic except that single layout warning. Any other
  diagnostic, including `unsupported-field`, still fails S2.

