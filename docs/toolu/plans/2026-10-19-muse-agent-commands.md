# Muse agent commands — Plan

**Date:** 2026-10-19   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-19-muse-agent-commands-design.md   **Topic:** Falconiere/toolu#548

## Evidence and approach

The spec settles the design. Each `plugins/toolu/agents/<name>.md` becomes a
Muse command whose `path` is that same file. The toolu `.muse-plugin/plugin.json`
holds those five commands plus the six toolu skills. A new xtask module,
`packaging_muse.rs`, is called from `packaging::package` and fails on drift.

Inspected:

- `crates/xtask/src/packaging.rs`: `package` and `check_manifests` do the
  lockstep and release checks.
- `crates/xtask/src/packaging_assets.rs`: `dirs`, `read_json`, and
  `check_agents` for the Codex TOML. The file is 307 lines, so the new check
  goes in a new module.
- `crates/xtask/src/packaging_catalog.rs`: `release_tracks`.
- `crates/xtask/src/tests/packaging_test.rs`: the tempdir `write` helper and
  `this_repository_is_packaged`.
- `crates/xtask/src/hooks_muse.rs`: #546's Muse hook judge, which is untouched.
- `.github/ci-paths.json`.

The live Muse 1.4.11 probes are recorded in the spec.

## Workstream summary

Gate module and tests, with the manifest and release tracking → live Muse
validation → docs → CI path groups → live TUI evidence → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Add crates/xtask/src/packaging_muse.rs (check_muse: lockstep version, release-please tracking, top-level key allow-list, no capabilities.agents, one command {id,path=agents/<id>.md} per agent, no stale agents/ entry, agents require a manifest) with tests/packaging_muse_test.rs over copies of the real plugins/toolu files (red first), wire it into packaging::package and its report line, add plugins/toolu/.muse-plugin/plugin.json (6 skills, 5 agent commands) and its release-please extra-file",
    "check": "cargo test -p xtask packaging && cargo xtask packaging | grep -q ' 5 Muse agent commands'",
    "ac_refs": ["AC-3", "AC-4", "AC-5"],
    "paths": [
      "crates/xtask/src/packaging_muse.rs",
      "crates/xtask/src/packaging.rs",
      "crates/xtask/src/packaging_assets.rs",
      "crates/xtask/src/packaging_catalog.rs",
      "crates/xtask/src/main.rs",
      "crates/xtask/src/tests/packaging_muse_test.rs",
      "crates/xtask/src/tests/packaging_test.rs",
      "plugins",
      "package.json",
      "release-please-config.json",
      ".claude-plugin/marketplace.json",
      ".agents/plugins/marketplace.json",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "the real plugins/toolu/agents/*.md, plugins/toolu/.muse-plugin/plugin.json, package.json and release-please-config.json copied into a tempdir and mutated: failing cases are a renamed agent, a removed entry, a stale agents/ entry, a wrong id, capabilities.agents, top-level agents and author, a missing manifest, a wrong version, an untracked release, invalid JSON and a non-array commands; passing cases are the unmutated copy (Ok(5)), an edited agent body with the manifest untouched, a .gitkeep-only agents/, a plugin with neither agents nor manifest, and a command entry outside agents/",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Validate the committed toolu Muse bundle with the real muse CLI: valid, zero diagnostics, the 5 agent commands pointing at agents/*.md, the 6 skills, and every skill directory clean under muse skills validate",
    "check": "muse plugins validate plugins/toolu --json | jq -e '.valid == true and (.diagnostics | length == 0) and ([.capabilities.commands[] | .id + \"=\" + .path] | sort == [\"architect=agents/architect.md\", \"deep-explore=agents/deep-explore.md\", \"implementer=agents/implementer.md\", \"quick-task=agents/quick-task.md\", \"research-agent=agents/research-agent.md\"]) and (.capabilities.skills | length == 6)' && for d in plugins/toolu/skills/*/; do muse skills validate \"$d\" --json | jq -e '.valid == true and (.diagnostics | length == 0)' || exit 1; done",
    "ac_refs": ["AC-1"],
    "depends_on": ["S1"],
    "paths": ["plugins/toolu"],
    "input": "plugins/toolu as committed, validated by Muse Code 1.4.11",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Docs: Muse section in plugins/toolu/README.md (mapping rule, /toolu:<agent>, current-thread note, validate command); Muse syntax per agent in plugins/toolu/skills/orchestrator/SKILL.md and both identical model-routing.md copies; AGENTS.md CI rust-group and release-only sentences",
    "check": "grep -q 'Muse' plugins/toolu/README.md && grep -q 'agents/<name>.md' plugins/toolu/README.md && grep -q 'Muse `/toolu:quick-task`' plugins/toolu/skills/orchestrator/SKILL.md && grep -q '/toolu:architect' plugins/toolu/skills/orchestrator/references/model-routing.md && cmp plugins/toolu/skills/orchestrator/references/model-routing.md plugins/delivery-flow/skills/delivery-flow/references/model-routing.md && grep -q 'plugins/\\*/.muse-plugin/plugin.json' AGENTS.md && cargo xtask check-markdown-cli && cargo xtask gate --only context-budget",
    "ac_refs": ["AC-6"],
    "depends_on": ["S1"],
    "paths": [
      "plugins/toolu/README.md",
      "plugins/toolu/skills/orchestrator",
      "plugins/delivery-flow/skills/delivery-flow/references/model-routing.md",
      "AGENTS.md",
      "docs",
      "tooling/conventions/markdown-cli.json",
      "plugins",
      "crates/xtask/src"
    ],
    "input": "the edited Markdown surfaces, judged by check-markdown-cli against docs/cli/commands.json",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "CI path groups: rust gains plugins/*/.muse-plugin/plugin.json and plugins/*/agents/*.md; releaseOnly.paths gains plugins/*/.muse-plugin/plugin.json",
    "check": "jq -e '(.groups.rust | index(\"plugins/*/.muse-plugin/plugin.json\") and index(\"plugins/*/agents/*.md\")) and (.releaseOnly.paths | index(\"plugins/*/.muse-plugin/plugin.json\"))' .github/ci-paths.json && cargo xtask gate --only ci-paths",
    "ac_refs": ["AC-5"],
    "depends_on": ["S1"],
    "paths": [".github", "plugins", "crates/xtask/src"],
    "input": ".github/ci-paths.json against the tracked files",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Live Muse TUI evidence: install plugins/toolu into a scratch XDG_DATA_HOME, run muse --provider echo --trust-workspace in tmux, capture the /toolu: menu, /toolu:architect and /toolu:quick-task expansions and /orchestrator naming Muse /toolu:quick-task; save to docs/toolu/evidence/2026-10-19-muse-agent-commands.txt; remove the scratch install",
    "check": "f=docs/toolu/evidence/2026-10-19-muse-agent-commands.txt; ! grep -q '/root/' $f && for c in architect deep-explore implementer quick-task research-agent; do grep -q \"/toolu:$c \" $f || exit 1; done && grep -q 'You are called when getting it' $f && grep -q 'You handle mechanical work' $f && grep -q 'Muse /toolu:quick-task' $f",
    "ac_refs": ["AC-2", "AC-7"],
    "depends_on": ["S2", "S3"],
    "paths": ["docs/toolu/evidence/2026-10-19-muse-agent-commands.txt"],
    "input": "plugins/toolu installed into Muse Code 1.4.11 with the echo provider, which returns the submitted prompt",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": ["AC-5"],
    "depends_on": ["S1", "S2", "S3", "S4", "S5"],
    "model": "inherit"
  }
]
```

## Critical files

- `crates/xtask/src/packaging_muse.rs` (new)
- `crates/xtask/src/tests/packaging_muse_test.rs` (new)
- `crates/xtask/src/packaging.rs`
- `plugins/toolu/.muse-plugin/plugin.json` (new)
- `release-please-config.json`
- `.github/ci-paths.json`
- `plugins/toolu/README.md`
- `plugins/toolu/skills/orchestrator/SKILL.md`
- `plugins/toolu/skills/orchestrator/references/model-routing.md`
- `plugins/delivery-flow/skills/delivery-flow/references/model-routing.md`
- `AGENTS.md`
- `docs/toolu/evidence/2026-10-19-muse-agent-commands.txt` (new)

## Verification

**End to end.**

- The real `muse` validates the committed bundle clean (S2).
- A real Muse TUI session lists the five commands and expands them to the agent
  text (S5).
- `/orchestrator` names the Muse syntax, which resolves (S5).

**Failure and boundary.** The S1 tests mutate copies of the real files for each
drift case in the spec's Failure modes, and check that each returns its message.
An edit to an agent's text passes, as do a `.gitkeep`-only `agents/` and a
plugin without agents or a manifest.

**Docs.** The README, the orchestrator skill, both identical `model-routing.md`
copies and AGENTS.md change together (S3). `check-markdown-cli` and
`context-budget` judge them. The full `cargo xtask gate` closes the plan (S6).

**Delivery.**

1. Commit scoped `feat(toolu): …` changes.
2. Run `toolu ledger run docs/toolu/plans/2026-10-19-muse-agent-commands.md --verify`
   over the whole branch diff.
3. Run `toolu-review:review` to write version-2 push-review state with zero
   findings.
4. `toolu ledger verdict status` must report `overall: ready`.
5. Rebase on `origin/main` if it moved, then push
   `feat/548-remap-the-5-toolu-agents`.
6. Open the PR against `main`. Its body starts with `Closes Falconiere/toolu#548`
   and `Part of Falconiere/toolu#539`, and carries the S2/S5 evidence.
7. `report pr-open --pr <n>`, then `report babysit --pr <n>` in place of
   starting babysit, as the epic brief requires.
