# Muse live end-to-end verification — Design

**Date:** 2026-10-20   **Status:** Approved   **Author:** epic worker (#553)   **Topic:** the epic #539 acceptance run: toolu installed, approved, enforcing and reporting in real Muse sessions

## Problem

Fixtures and `muse plugins hook test` prove the Muse contract piece by piece
(#540–#552, #554). Only a real session proves the wiring between them: manifest
loading, approval activation, environment inheritance, `PATH` resolution, and
dispatch without matchers under real tool traffic, internal tools included.
Epic #539 cannot be called done until a session run shows toolu installing,
approving, enforcing and reporting on Muse as it does on Claude Code and Codex.

## Non-Goals

1. No `cargo xtask host-smoke --host muse` driver. That is #552.
2. No product code unless the run finds a wiring-only bug that belongs to no
   other sub-issue. A deviation owned by a sub-issue is filed against it (its
   own issue, `fix-forward`), not fixed here.
3. No change to the hub's `mcp-tools`/`agent-tier` behaviour (#554). The run
   records what those entries do today.
4. No marketplace or GitHub-release distribution test. The release that carries
   the Muse manifests is not out yet. Seeding the checkout follows #549's
   verified practice.

## Architecture

The run uses one scratch "clean machine" and drives it by hand, with every
command captured:

- **Binary.** `cargo build -p toolu-cli` at `origin/main` (04f29e95, version
  8.0.4, all merged Muse work) is the release candidate. It is installed as
  `$HOME/.local/bin/toolu`, the install directory `install.sh` uses.
- **Isolation.** Every command runs under `env -i` with `HOME`,
  `XDG_CONFIG_HOME` and `XDG_DATA_HOME` inside the scratch directory. `PATH` is
  `$HOME/.local/bin:<private muse/node/git bin>:/usr/bin:/bin`. The run sits
  inside `unshare -m`, with an empty directory bind-mounted over
  `/usr/local/bin`. That hides this host's older `/usr/local/bin/toolu`, which
  the launcher checks before `~/.local/bin`. The only Muse state copied in is
  the provider credential (`auth.json`, mode 0600), which never appears in
  evidence.
- **Plugins.** The checkout `$XDG_DATA_HOME/toolu/muse-marketplace` is seeded
  as a shallow clone of `origin/main`. It gets a local tag `v8.0.4` at HEAD and
  the GitHub origin URL, as in #549's evidence. Then `toolu plugins install
  --host muse` runs: first `toolu` alone without `--approve` (check 8), then
  every plugin with `--approve`.
- **Sessions.** `muse exec --json` in a git project. Each check has its own
  prompt and transcript. A hook's verdict is read from the JSONL tool events:
  `tool blocked by hook: …` or an executed result. On-disk effects are checked
  independently: file bytes, `HEAD`, refs in the bare remote, the gate file,
  `toolu status`.
- **Project config.** `.agents/toolu.config.json` (Muse's project directory,
  the host-smoke `LIMITS` shape) sets `protectedFiles`, `commitGate`,
  `qualityGate` and `pushReview` to `block`, plus a 3-line TypeScript
  `maxFileLines`, so each probe is deterministic. Check 1 runs once more under
  the default preset to show the shipped default also blocks on Muse: its
  `ask` encodes as a block there.
- **Decided readings.** Jev was asked about the two ambiguous checks.
  - Check 3 ("commit with approval"): the commit is blocked while the quality
    gate is red, and the same commit is allowed once the agent fixes the
    violation (Jev choice `fix`, 0.95).
  - Check 4 ("record a waiver"): the push is blocked with no review. Running
    `toolu review write-state --findings-count 0` records a clean review of
    the exact diff, and the push is then allowed (Jev `state` 0.54 against
    `none` 0.45). The ask-mode waiver of `docs/config.md` cannot be granted on
    Muse by design: the Muse encoder turns `ask` into a block (#544), so no
    approved push ever promotes a pending waiver. The evidence states this, and
    shows ask mode blocking.
- **Evidence.** `docs/toolu/evidence/2026-10-20-muse-live-e2e.md` holds the
  setup, a verdict table, and one redacted transcript excerpt per check, in
  `text` fences. Redaction replaces the scratch path with `$SCRATCH` and drops
  session ids, the account and any credential. A comment on #553 links it.

## Interfaces / Schema

- Evidence file: `docs/toolu/evidence/2026-10-20-muse-live-e2e.md`. It has a
  `## Verdicts` table with one row per check, `| <n> | <check> | PASS\|FAIL |
  <observed> |`, then `## Check <n>` sections with the commands, exit codes
  and the tool events quoted.
- Project config written for the run (`.agents/toolu.config.json`):
  `{"gates":{"protectedFiles":{"mode":"block"},"commitGate":{"mode":"block"},"qualityGate":{"mode":"block"},"pushReview":{"mode":"block"}},"lang":{"ts":{"maxFileLines":3}}}`.
- No code interface changes.

## Failure modes and edge cases

- **Model ignores the prompt** (no tool call, or a different tool). The check
  is inconclusive, not a pass. Rerun with a more literal prompt, at most twice,
  and record every attempt.
- **Hook reported `failed` in the JSONL** (exit 2 with JSON, empty stderr,
  timeout). That is a deviation, even when the tool happened to be blocked: it
  is the fail-open class the epic forbids.
- **Hooks inert after approval** (`review_needed`/`trusted_disabled` in
  `muse plugins inspect`). It is recorded with the inspect output and filed
  against #549.
- **Check 6, binary renamed.** The block must come from the resolver's
  `no usable native toolu` advisory. Silence, an allowed call or a `failed` hook
  fails the check. The binary is restored afterwards and a control call shows
  enforcement is back.
- **`/usr/local/bin` not masked.** The launcher would pick the older host
  binary and check 6 would test nothing. Before any session, `command -v
  toolu`, the resolver's own choice (`toolu --version`) and the masked
  directory's listing are recorded.
- **#554 still open.** If `mcp-tools` or `agent-tier` reports a failure in
  check 5, the evidence records it and attributes it to #554. It is not fixed
  here.
- **A deviation found.** The bug is filed against its owning sub-issue and the
  verdict row says FAIL with the issue link. Under the issue's fix-forward rule,
  #553 stays open: the PR then says `Part of` rather than `Closes`.

## Acceptance criteria

- **AC-1:** In an approved session, a `.env` write is blocked. The tool event
  reads `tool blocked by hook:`, its reason names the protected-files rule, and
  `.env` is byte-identical afterwards. This holds under both the block config
  and the default preset.
- **AC-2:** A session edit that breaks `maxFileLines` in `sample.ts` leaves the
  gate file failing for it, and `toolu status` in the project shows the
  failing quality gate.
- **AC-3:** With the gate red, the session's `git commit` is blocked and `HEAD`
  is unchanged. After the agent fixes the file, the same commit runs and `HEAD`
  advances.
- **AC-4:** On a branch ahead of `main` with a bare `origin`, a session
  `git push` is blocked by push review and the remote gains no ref. After
  `toolu review write-state --findings-count 0`, the push succeeds and the
  remote has the branch.
- **AC-5:** Two sessions run with no hook `failed` outcome and no unexpected
  block in their JSONL:
  - a prompt that needs no workspace tool (a plain answer), so only Muse's
    own internal tools can run;
  - a routine read/edit/bash session, interleaved with internal tools such as
    `submit_reminder_decision`, that completes its work.

  The tool names each session used are listed from its events.
- **AC-6:** With `$HOME/.local/bin/toolu` renamed away mid-run, a session
  `bash` call is blocked, with a reason that carries the resolver's install
  advisory (`no usable native toolu … Install toolu with …`). After it is
  restored, the next call is gated normally.
- **AC-7:** `toolu doctor` in the project reports no failing item. It names
  the Muse config root (`$XDG_CONFIG_HOME/muse`) and every installed plugin as
  approved.
- **AC-8:** With `toolu` installed but not approved
  (`muse plugins inspect toolu` shows `review_needed`), a `.env` write
  proceeds. After `toolu plugins install toolu --host muse --approve`
  (`trusted_enabled`), the same write is blocked.
- **AC-9:** The evidence file records the setup and a PASS/FAIL verdict with a
  redacted excerpt for each of AC-1 to AC-8. It contains no scratch path,
  session id or credential, and the run's verdicts are linked from #553. When
  every row passes, it states that the epic's enforcement contract is verified
  live.
- **AC-10:** No bug filed by this run is open when the PR closes #553. Every
  FAIL row links the issue it was filed under. The PR body says `Closes
  Falconiere/toolu#553` only when all eight rows pass after the final rerun;
  otherwise it says `Part of` and #553 stays open.

## Acceptance evidence

| AC | Real input | Expected observable | Boundary / failure | Runnable check |
|----|------------|---------------------|--------------------|----------------|
| AC-1 | Muse 1.4.11 session, project `.env` = `KEEP=1` | `tool blocked by hook` + protected reason; `.env` unchanged | default preset (`ask` → block on Muse) | evidence `## Check 1` + verdict row PASS |
| AC-2 | `sample.ts` grown past 3 lines by the session | gate file `failing`, `toolu status` names it | — | evidence `## Check 2` |
| AC-3 | staged `sample.ts`, red gate | commit blocked, `HEAD` same; after fix `HEAD` moves | blocked commit must not move `HEAD` | evidence `## Check 3` (`git rev-parse` before/after) |
| AC-4 | `feat/push-smoke` + bare `origin` | push blocked, no remote ref; then pushed | ask mode also blocks (no waiver on Muse) | evidence `## Check 4` (`git for-each-ref` on the remote) |
| AC-5 | routine read/edit/bash prompt | no `failed` hook event | internal tools present in JSONL | evidence `## Check 5` (event counts) |
| AC-6 | `$HOME/.local/bin/toolu` renamed, `/usr/local/bin` masked | block with install advisory | restore → gated again | evidence `## Check 6` |
| AC-7 | `toolu doctor` in project | no failing line, Muse roots | — | evidence `## Check 7` |
| AC-8 | `toolu` installed without `--approve` | `.env` write proceeds; after approve, blocked | `review_needed` → `trusted_enabled` | evidence `## Check 8` |
| AC-9 | the committed evidence file | 8 verdict rows, no scratch path or ids | redaction | `grep` checks in the plan's evidence step |
| AC-10 | `gh issue list --label` / FAIL rows of the table | no FAIL row without an issue link; no open run bug when closing | a FAIL row → `Part of` | evidence-step `grep -c '| PASS |'` = 8, or a linked issue per FAIL |

The sessions spend model credits and are not deterministic, so they run once
by hand, as `host-smoke` does. The ledger checks the committed evidence file;
the transcripts are the proof.

## Documentation impact

A new evidence file, `docs/toolu/evidence/2026-10-20-muse-live-e2e.md`. No user-facing
doc changes unless the run finds wrong documentation. That would be fixed
here if wiring-only, or filed against #551.

## Open Questions

None blocking. If the run finds a deviation, the PR keeps #553 open (`Part of
Falconiere/toolu#553`) as the issue's fix-forward rule says. The worker decides
this from the verdict table; it is not a design question.

## Spec review

Round 1 (Jev coverage check: `noul` 0.42; weakest requirement "zero open bugs" 0.81):

- Acceptance criteria: 🔴 blocker: the issue's "zero open bugs from this run" and "the enforcement contract is marked verified" had no AC. Fixed: AC-10 added, and AC-9 now states the contract is verified.
- Acceptance criteria: 🟡 should-fix: check 5 asks for a prompt that triggers internal tools only, and AC-5 covered only interleaved routine work. Fixed: AC-5 now runs both sessions.
- Architecture: 🔵 consider: the brief's PR template says `Closes`, while the issue keeps #553 open on a deviation. Resolved in AC-10: `Closes` only when every row passes.

Round 2: every section is filled, and each AC has a real input, an observable result, a boundary and a check. **Status:** Approved.
