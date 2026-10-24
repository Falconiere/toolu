# Epic-orchestrator skill: preflight and waiting on all four hosts — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-epic-skill-four-host-preflight-design.md   **Topic:** Falconiere/toolu#605

## Evidence and approach

The outcome is that the skill's preflight reports each worker host's
readiness (CLI, login, worker plugins, effective hooks) and catches the
unapproved Muse plugin. Waiting covers all four orchestrator hosts. The state
directory is documented and pinned per host. Inspected for this plan:

- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`, §1 and §4,
  plus its description and options.
- `plugins/epic-orchestrator/README.md`: intro, install, waiting paragraph,
  and the State directory table at lines 179–184.
- `docs/epic-orchestrator/README.md`: the wait comment at line 37.
- `crates/epic-orchestrator/src/refs.rs` (`epics_home`) and
  `src/tests/refs_test.rs`
  (`epics_home_follows_the_host_override_and_epic_state_home`).
- `crates/toolu/src/doctor/inventory.rs` (`muse_notice`): doctor already
  prints the Muse approval fix.
- `tooling/conventions/markdown-cli.json`: its `external` list has `claude`,
  `codex` and `muse`, but not `cursor-agent`, `herdr` or `security`. The skill
  keeps those in inline spans, which the check doesn't run.

The approach is skill-only. Preflight runs existing commands. Rust gets one
doc comment and one characterization test. No CLI surface changes, so neither
`docs-cli` nor the contract snapshot changes.

## Workstream summary

state-home test and doc comment → SKILL.md (preflight, wait, description) →
READMEs → live four-host preflight evidence → full gate.

## Steps (machine-readable)

```json
[
  {"id": "s1", "title": "Pin epics_home for each orchestrator host's shell environment; state the rule in its doc comment", "check": "cargo test -p toolu-epic-orchestrator refs::tests::epics_home", "ac_refs": ["AC-3"], "paths": ["crates/epic-orchestrator/src/refs.rs", "crates/epic-orchestrator/src/tests/refs_test.rs"], "input": "the variables each host's shell exported in the 2026-10-23 probe: Codex CODEX_SESSION_ID/CODEX_THREAD_ID/CODEX_SANDBOX without CODEX_HOME, Cursor CURSOR_AGENT/CURSOR_CONVERSATION_ID, Muse MUSE_SESSION_ID/MUSE_THREAD_ID, Claude CLAUDECODE/CLAUDE_CODE_SESSION_ID; plus EPIC_STATE_HOME, TOOLU_HOST_OVERRIDE=codex without CODEX_HOME, and CODEX_HOME", "model": "inherit"},
  {"id": "s2", "title": "SKILL.md: four-host description and options, per-host preflight with fixes and sandbox check, four-host wait", "check": "cargo xtask check-markdown-cli && grep -q '^description:.*Cursor Agent.*Muse Code' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -qF '(hosts `claude`, `codex`, `cursor`, `muse`' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -qF 'muse plugins approve' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -qF 'security unlock-keychain' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -qF 'toolu plugins install delivery-flow --host' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -qF 'Operation not permitted' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -qF -- '--disable-sandbox' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md && grep -A3 -F -e '- **Cursor Agent:**' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md | grep -qF -- '--max-seconds 480' && grep -A3 -F -e '- **Muse Code:**' plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md | grep -qF -- '--max-seconds 480'", "ac_refs": ["AC-2", "AC-4", "AC-6", "AC-7"], "depends_on": ["s1"], "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "tooling/conventions/markdown-cli.json", "docs/cli/commands.json"], "input": "the skill text, judged against docs/cli/commands.json by check-markdown-cli", "model": "inherit"},
  {"id": "s3", "title": "Plugin and docs READMEs: four hosts as orchestrator, Muse install, four-host waiting, real state-directory rule and sandbox note", "check": "cargo xtask check-markdown-cli && grep -qF '**Cursor Agent**, and **Muse Code**, both' plugins/epic-orchestrator/README.md && grep -qF '### Muse Code' plugins/epic-orchestrator/README.md && grep -qF 'on Codex, Cursor Agent and Muse Code' plugins/epic-orchestrator/README.md && grep -qF 'Claude Code, Cursor Agent, Muse Code' plugins/epic-orchestrator/README.md && grep -qF 'Operation not permitted' plugins/epic-orchestrator/README.md && grep -qF 'Codex, Cursor Agent and Muse Code: a loop' docs/epic-orchestrator/README.md", "ac_refs": ["AC-4", "AC-6", "AC-7"], "depends_on": ["s2"], "paths": ["plugins/epic-orchestrator/README.md", "docs/epic-orchestrator/README.md", "docs/cli/commands.json"], "input": "the README text", "model": "inherit"},
  {"id": "s4", "title": "Live preflight from inside each of the four hosts, the skill text driving a Muse agent, the sandboxed boundary; worker brief invocations unchanged", "check": "bash /tmp/w605-preflight/check-evidence.sh && cargo test -p toolu-epic-orchestrator skill_syntax_matches_every_host && cargo test -p toolu-epic-orchestrator brief", "ac_refs": ["AC-1", "AC-5", "AC-7"], "depends_on": ["s2"], "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md", "crates/epic-orchestrator/src/hosts.rs", "crates/epic-orchestrator/src/tests/hosts_test.rs", "crates/epic-orchestrator/src/launch/brief.rs"], "input": "/tmp/w605-preflight/preflight.sh (the skill's preflight commands verbatim) run in a Claude Code Bash call, codex exec --dangerously-bypass-approvals-and-sandbox, cursor-agent -p --force and muse exec --disable-sandbox; muse exec --disable-sandbox given only SKILL.md section 1; muse exec with its default sandbox as the boundary case; transcripts under /tmp/w605-preflight/evidence/", "model": "inherit"},
  {"id": "s5", "title": "Full quality gate", "check": "toolu epic job -- /tmp/w605-gate/gate.sh", "ac_refs": ["AC-6"], "depends_on": ["s1", "s2", "s3", "s4"], "paths": ["crates/epic-orchestrator/src/refs.rs", "crates/epic-orchestrator/src/tests/refs_test.rs", "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "plugins/epic-orchestrator/README.md", "docs/epic-orchestrator/README.md"], "input": "the whole branch", "model": "inherit"}
]
```

## Critical files

- `crates/epic-orchestrator/src/refs.rs`: `epics_home` doc comment.
- `crates/epic-orchestrator/src/tests/refs_test.rs`: new
  `epics_home_for_each_orchestrator_host` test.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`
- `plugins/epic-orchestrator/README.md`
- `docs/epic-orchestrator/README.md`

## Verification

- **End to end (AC-1):** `/tmp/w605-preflight/preflight.sh` mirrors the
  skill's §1 commands and runs inside each host. Expected on this machine:
  - Claude Code is ready, with a retired-plugin skew note;
  - Codex is ready;
  - Cursor is not ready, with `toolu plugins install delivery-flow --host
    cursor`;
  - Muse is not ready, with `muse plugins approve toolu`.

  A Muse Code agent given only `SKILL.md` §1 must name
  `muse plugins approve toolu` (AC-1). Boundary case (AC-7): under
  `muse exec` with its default sandbox, `herdr status` and
  `toolu epic status` fail with `Operation not permitted`. `check-evidence.sh` asserts these lines in the saved transcripts.
- **Waiting (AC-2):** §4 matches the 2026-10-23 foreground-duration probe for
  Cursor Agent and Muse Code. The probe's log and reply are quoted in the PR
  body.
- **Failure propagation:** every check in the skill names its stop condition
  and its fix. Doctor's exit 0 on `warn plugins` is handled by keying on the
  line.
- **Docs:** steps s2 and s3, with `check-markdown-cli`.
- **Delivery:**
  1. scoped commit;
  2. `toolu ledger run docs/toolu/plans/2026-10-23-epic-skill-four-host-preflight.md --verify`;
  3. `toolu-review:review`;
  4. `toolu ledger verdict status` shows `overall: ready`;
  5. push;
  6. PR with `Closes Falconiere/toolu#605` and `Part of Falconiere/toolu#591`;
  7. `toolu epic report babysit --pr <n>`, in place of the babysit skill, as
     the epic brief requires.

## Deviations

- s2 check, fixed at execution: the Options grep required `)` right after
  `muse`, but the line now adds the default host after it. The bullet greps passed a
  pattern starting with `-` without `-e`, so grep read it as a flag. The
  content was unchanged; only the checks were corrected.
- s2 and s4: Cursor's transcript showed that the plugins check must also
  require `toolu plugins list` to succeed. Its shell resolves a stale
  `/usr/local/bin/toolu` 8.1.3, which exits 64 for `--host cursor`. Both
  `SKILL.md` step 2 and the mirror script now require success, and the
  Cursor transcript was re-run.
- s2, from self-review: the session check names the host
  (`toolu --host <host> doctor`), because toolu's detection can't identify a
  Cursor shell, which exports only `CURSOR_AGENT`.
- s5: on this Mac, the full `cargo xtask gate` can't pass its `tests` and
  `coverage` steps. `/opt/homebrew/bin/toolu` breaks the `launcher`, jev and
  `startup::native` tests, and the live epic engine breaks the contract
  schema test. Both reproduce on a clean `origin/main` (local-gate memory,
  2026-10-23). The check is now `/tmp/w605-gate/gate.sh`:
  - every other gate step, via `--only`;
  - plus `cargo test -p toolu-epic-orchestrator`, the only Rust package
    changed;
  - with `TMPDIR=/private/tmp`, `npm_config_store_dir` unset, and a temporary
    `TOOLU_RESOURCE_HOME`.

  CI runs the full gate.

## Plan review (2026-10-23)

- Every spec AC (AC-1 to AC-7) is mapped by at least one step's `ac_refs`.
- s2: 🟡 should-fix: the wait check counted `--max-seconds 480` anywhere in
  the file, which did not prove the Cursor and Muse bullets. It now greps
  each bullet's own lines. Fixed.
- s4: 🟡 should-fix: the evidence checker did not cover the skill-text Muse
  run. `check-evidence.sh` now requires `muse plugins approve toolu` in
  `muse-skill.out`. Fixed.
- s1: 🔵 consider: the test pins behavior that already exists. Its red
  evidence is the README claim it contradicts: a Codex shell exports no
  `CODEX_HOME`, so state goes to `~/.claude/epics`, not to
  `~/.codex/toolu/epics`. Recorded at execution.
- Jev step and requirement alignment scored 1.09 of 2 before these fixes.
  Both findings it pointed toward were applied.

**Status:** Approved
