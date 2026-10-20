# Live run: toolu end to end on Muse (#553, epic #539)

2026-10-20, Muse Code 1.4.11 (`1.4.11-R6667.1`), model sessions through `muse exec
--json`. The `toolu` under test is `cargo build -p toolu-cli` at `origin/main`
04f29e95 (8.0.4, every merged Muse sub-issue). The machine-readable record is
[`2026-10-20-muse-live-e2e.json`](2026-10-20-muse-live-e2e.json). A script derived
it from the raw `muse exec --json` records, Muse's own session logs (one
`hook_run_terminal` record per hook run, subagents included) and the on-disk
captures. Every `facts` boolean in it is computed from those files, not typed.
`$SCRATCH` stands for the scratch directory. Session, call and command ids are
dropped.

## Verdicts

| # | Check | Verdict | Observed |
|---|-------|---------|----------|
| 1 | denied `.env` write | PASS | `edit_file` and `bash` writes blocked; reason names `gates.protectedFiles`; `.env` byte-identical (block config and default preset) |
| 2 | quality violation recorded | PASS | `write_file` grows `sample.ts` to 6 lines; post-tools `system_message` "QUALITY VIOLATION"; gate file `failing` for `sample.ts`; `toolu status` shows it with `host: muse` |
| 3 | commit blocked, then allowed | PASS | `git commit` blocked by the red quality gate, `HEAD` unchanged; after the agent's fix the same commit lands |
| 4 | push blocked, then allowed | FAIL #571 | push blocked with no review and remote unchanged; push lands after a review is recorded, but the agent cannot record it in-session (`.agents` is read-only in Muse's sandbox) |
| 5 | internal tools, no hook failures | PASS | no-tool prompt (only `submit_reminder_decision` ran) and routine read/edit/bash session: 0 failed hook runs, 0 blocks |
| 6 | binary removed mid-session | PASS | next `bash` call blocked with the resolver's install advisory; after restore, calls are gated again |
| 7 | `toolu doctor` all-green | FAIL #572 | from a terminal: every line `ok`, exit 0, Muse roots; in-session (the Muse `doctor` command): false `fail config` and "hooks are installed but inert" |
| 8 | unapproved plugin inert, approval activates | PASS | 13 hooks `review_needed`: `.env` write runs, zero hook runs; after `--approve` (13 `trusted_enabled`) the same write is blocked |

The **enforcement contract** held live in every check:
- protected writes, red-gate commits and unreviewed pushes were blocked;
- a missing binary blocked `PreToolUse` with the install advisory;
- `PostToolUse` and context events reported through `systemMessage`;
- no hook run in any of the 18 sessions ended `failed`.

Acceptance is not complete. Two in-session defects were found and filed
(fix-forward):
- [#571](https://github.com/Falconiere/toolu/issues/571), owner #542: agent-run
  toolu commands cannot write Muse project state.
- [#572](https://github.com/Falconiere/toolu/issues/572), owner #550: in-session
  doctor reports false failures.

#553 stays open until checks 4 and 7 rerun green.

## Clean machine

```text
# every command: unshare -m (an empty dir bind-mounted over /usr/local/bin, hiding
# this host's older /usr/local/bin/toolu, which the launcher checks before ~/.local/bin),
# then env -i HOME=$SCRATCH/run/home XDG_CONFIG_HOME=$HOME/.config
# XDG_DATA_HOME=$HOME/.local/share PATH=$HOME/.local/bin:<muse, node, npm>:/usr/bin:/bin
$ ls -A /usr/local/bin
[empty]
$ command -v toolu && toolu --version && toolu --hook-protocol
$SCRATCH/run/home/.local/bin/toolu
toolu 8.0.4
2
$ muse --version
Muse Code 1.4.11 (1.4.11-R6667.1)
# only the Muse provider credential (auth.json, 0600) was copied into $HOME/.config/muse
# seed (the #549 practice): shallow clone of GitHub main at 04f29e95 into
# $XDG_DATA_HOME/toolu/muse-marketplace, local tag v8.0.4 at HEAD, origin = GitHub
```

Projects use the host-smoke shape: `.env` = `KEEP=1`, a one-line `sample.ts`,
`package.json`, and `package-lock.json`. `.agents/toolu.config.json` holds
`{"gates":{"protectedFiles":{"mode":"block"},"commitGate":{"mode":"block"},"qualityGate":{"mode":"block"},"pushReview":{"mode":"block"}},"lang":{"ts":{"maxFileLines":3}}}`.
`default-proj` has no config, so the shipped preset applies. Later the run added
`herdr` and `ast-grep` to `PATH`, which doctor's `tools` check requires.

## Check 8 — trust step

```text
$ toolu plugins install toolu --host muse
  + toolu            installed; awaiting approval: muse plugins approve toolu
[exit 0]
$ muse plugins inspect toolu --json   # runtime_capabilities[].status
{"review_needed":13}
# session c8-unapproved (default-proj): "Run exactly this ...: printf 'ADDED=1\n' >> .env"
[bash/success] "command": "printf 'ADDED=1\\n' >> .env", "exit_code": 0
hook runs in the session log: 0
$ cat .env
KEEP=1
ADDED=1
$ toolu plugins install toolu --host muse --approve
  = toolu            already installed at 8.0.4; approved
[exit 0]
$ muse plugins inspect toolu --json
{"trusted_enabled":13}
# session c8-approved, same prompt, .env reset to KEEP=1
tool blocked by hook: This command would WRITE to .env, a protected path (matches ".env"). This is a secrets file. ... Blocked by gates.protectedFiles.mode='block' (see plugins/toolu/hooks/docs/gates.md).
PreToolUse plugin:toolu:pre-tools status=blocked
$ cat .env
KEEP=1
```

The rest of the plugins followed: `toolu plugins install --host muse --approve`
installed all 11 (`installed; approved`; `brainstorm` and `delivery-flow` have no
hooks), and `toolu plugins list --host muse` listed all 12 at 8.0.4. Every hook
capability of every plugin was then `trusted_enabled`.

## Check 1 — denied `.env` write

```text
# session c1-block (proj, block config): "Use the edit or write file tool (not bash) to change .env ..."
tool blocked by hook: Claude is trying to edit .env, a protected path (matches ".env"). ... Blocked by gates.protectedFiles.mode='block' (see plugins/toolu/hooks/docs/gates.md).
tool blocked by hook: Claude is trying to edit .env, a protected path (matches ".env"). ...
PreToolUse plugin:toolu:pre-tools status=blocked  x2
.env sha256 before = after = 01e64c39a868473aadad05e565172f129239ab3fbd4494bb2bb4f6d2e4429746
```

Under the default preset (`default-proj`, check 8's approved session),
`protectedFiles` is `ask`. Muse cannot prompt, so a guardrail's `ask` degrades
to `block` there. The bash write was blocked with the same rule.

## Check 2 — quality violation recorded

```text
# session c2-edit: write_file makes sample.ts six one-statement lines
[write_file/success] wrote 120 bytes to $SCRATCH/run/home/proj/sample.ts
PostToolUse plugin:toolu:post-tools status=completed
  system_message: QUALITY VIOLATION — fix before proceeding:
  TS file exceeds 3-line limit: sample.ts (6 code lines, blanks/comments excluded) — split into smaller modules
$ toolu status --host muse   (in proj)
"gate": {"entries": [{"file": "sample.ts", "source": "ts-quality-hook", "reason": "Post-edit quality violation(s) detected", ...}], "status": "failing"},
"host": "muse"
```

## Check 3 — commit blocked, then allowed

```text
$ git rev-parse HEAD
6afbc75a1a7e11ffa4617ca14d32bc16a357c043
# session c3-commit-red: (1) toolu status  (2) git add sample.ts && git commit -m 'feat: six constants'
[bash/success] toolu status -> "host": "muse", "gate": {"status": "failing", ...}
tool blocked by hook: BLOCKED: quality gate failing — fix the violations before committing or pushing.
Post-edit quality violation(s) detected
TS file exceeds 3-line limit: sample.ts (6 code lines, blanks/comments excluded) — split into smaller modules
$ git rev-parse HEAD
6afbc75a1a7e11ffa4617ca14d32bc16a357c043
# session c3-commit-green: write_file sample.ts back to two lines, then the same commit
[write_file/success] wrote 40 bytes to $SCRATCH/run/home/proj/sample.ts
[bash/success] "[main 2f5b69b] feat: two constants\n 1 file changed, 2 insertions(+), 1 deletion(-)"
$ cat .agents/tmp/quality-gate-status.json
{"status": "passing", "source": "ts-quality-hook", ...}
```

"With approval" is read as the gate cleared by fixing the violation. That is
toolu's only commit approval: `qualityGate` blocks `git commit` while red.

## Check 4 — push review (FAIL, #571)

```text
# push-proj: feat/push-smoke one clean commit ahead of main, bare origin holding only main
# session c4-push-blocked: git push -u origin feat/push-smoke
tool blocked by hook: Code review required before push (diff SHA 0b17edb9ecf389e73ef179ab3b6362ac12fba89c, base main). ...
$ git -C remote.git for-each-ref
refs/heads/main
# a second clean commit (e07cd6c); session c4-push-reviewed: (1) push (2) toolu review write-state --findings-count 0 (3) push
tool blocked by hook: Code review required before push (diff SHA d62aef8f363d73b522a1ecde66420918a1e508fc, base main). ...
[bash/failure] toolu review write-state --findings-count 0 -> exit 1
  review: cannot atomically write $SCRATCH/run/home/push-proj/.agents/tmp/push-review/feat_push-smoke.json
tool blocked by hook: Code review required before push ...
# session c4-sandbox-dirs: mkdir -p <d>/x for each candidate directory, from the agent's bash
.agents/x read-only
.muse/x writable
.git/x writable
.toolu/x writable
.claude/x writable
.codex/x writable
plain/x writable
# the same write-state from the user's terminal (outside Muse's sandbox)
$ toolu review write-state --findings-count 0
$SCRATCH/run/home/push-proj/.agents/tmp/push-review/feat_push-smoke.json
[exit 0]
# session c4-push-allowed: git push origin feat/push-smoke
[bash/success] "To $SCRATCH/run/home/remote.git\n   8298d72..e07cd6c  feat/push-smoke -> feat/push-smoke"
$ git -C remote.git rev-parse refs/heads/feat/push-smoke
e07cd6c4f4acc0151cab458874fd7d0f371abc4f   (= local HEAD)
```

Enforcement is right: no unreviewed push got through, and a recorded review
lets the push through. The in-session half fails. Muse's OS sandbox mounts the
project's `.agents/` read-only for the agent's tools, and toolu keeps Muse
project state in `.agents/tmp` (#542). So the `toolu-review:review` skill's
`write-state` cannot run in a Muse session, and neither can `toolu ledger run`:
`toolu ledger path` is `.agents/tmp/plan-ledger/main.json`. Filed as
[#571](https://github.com/Falconiere/toolu/issues/571).

The "waiver" of the issue text is the recorded review. `pushReview` in `ask`
mode never pends a waiver on Muse. In session `c4-push-ask` the push went
through, with the review requirement delivered as the pre-tools
`system_message`. This is the documented degrade
(`crates/core/runtime/src/config/gate_mode.rs`): a judgement gate's `ask`
becomes `advise` on a host that cannot prompt, as on Codex. It is not a
fail-open. The spec's "ask mode also blocks" was wrong, and the plan's
Deviations section records it.

## Check 5 — internal tools, no hook failures

```text
# session c5-no-tools: "Without using any workspace tool ..., what is 17 times 3?"
answer: 17 times 3 is 51.
tools seen: submit_reminder_decision (x2, in Muse's reminder subagents)
hook runs: SessionStart 19, UserPromptSubmit 2, PreToolUse 6, PostToolUse 2 — all completed, 0 failed
# session c5-routine: read_file sample.ts; edit_file (append a comment); bash git status --short; bash ls
[read_file/success] [edit_file/success] [bash/success] [bash/success]
tools seen: bash, edit_file, read_file, submit_reminder_decision
hook runs: SessionStart 19, UserPromptSubmit 2, PreToolUse 18, PostToolUse 6 — all completed, 0 failed
```

## Check 6 — binary removed mid-session

```text
# session c6-renamed: (1) touch started && sleep 40  (2) echo after-rename
# the harness waited for `started` (9 s), then: mv $HOME/.local/bin/toolu $HOME/.local/bin/toolu.renamed
[bash/success] touch started && sleep 40
PostToolUse plugin:toolu:post-tools completed, system_message: toolu plugin: no usable native toolu - checked TOOLU_BIN, ...
tool blocked by hook: blocked: toolu plugin: no usable native toolu - checked TOOLU_BIN, /opt/homebrew/bin, /usr/local/bin, /home/linuxbrew/.linuxbrew/bin, ~/.local/bin and PATH. If TOOLU_BIN is set, it must point to a native toolu and is never replaced. Install toolu with: curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash, or: brew install falconiere/tap/toolu. Then restart the session. See docs/install.md.
PreToolUse agent-tier, mcp-tools, pre-tools: status=blocked exit=2 on 24 calls each (internal tool calls included), 0 failed
# restore: mv toolu.renamed toolu; session c6-restored: (1) echo restored  (2) printf 'ADDED=1\n' >> .env
[bash/success] restored
tool blocked by hook: This command would WRITE to .env, a protected path ...
```

The `PostToolUse` advisory after the rename is the launcher's designed
context-event path. It matches the Claude Code `hooks.json` launcher byte for
byte.

## Check 7 — doctor (FAIL, #572)

```text
$ toolu doctor --host muse   (user's terminal, in proj)
ok binary: toolu 8.0.4 protocol 2 at $SCRATCH/run/home/.local/bin/toolu; upgrade with: curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
ok reachability: native toolu at $SCRATCH/run/home/.local/bin/toolu
ok host: muse config $SCRATCH/run/home/.config/muse project $SCRATCH/run/home/proj
ok config: config is valid
ok plugins: 12 toolu plugins
ok skew: plugin versions match
ok registry: no findings
ok tools: required tools are present
[exit 0]
# session c7-doctor-session: toolu doctor (what the Muse doctor command runs)
ok host: muse config $SCRATCH/run/home/.config/muse project $SCRATCH/run/home/proj
fail config: epic secrets: cannot read user-only secrets.json
warn plugins: ast-grep hooks are installed but inert; muse plugins approve ast-grep; ... toolu hooks are installed but inert; muse plugins approve toolu; ...
toolu doctor: 1 check failed            (exit 1; the same session ran 32 hooks, all completed)
# session c7-sandbox-probe: from the agent's bash
$HOME/.config/muse/settings.json absent-or-hidden
tbh: cannot read $HOME/.config/muse/settings.json: Permission denied (os error 13)
muse plugins inspect toolu --json -> 13 x "status": "review_needed"
```

Muse's sandbox hides `~/.config/muse` from the agent's shell. In-session
`muse plugins inspect` therefore reports every capability `review_needed`, and
doctor's #550 trust check calls running hooks inert. An unreadable user config
path becomes a secrets `fail`. Filed as
[#572](https://github.com/Falconiere/toolu/issues/572).

## Observations (not defects)

- The protected-files reason for a structured edit says "Claude is trying to edit"
  on every host (`crates/core/engine/src/gates/protected_files.rs`). The wording
  predates Muse and is not host-specific.
- Muse reports `multiple-manifests` (a warning) for each plugin. It selects
  `.muse-plugin/plugin.json` over the Claude and Codex manifests, as #541 recorded.
- Hooks run outside Muse's sandbox. The gate file, telemetry and the
  `push-review/` state written from the terminal all landed in `.agents/tmp`, and
  every hook read them.
