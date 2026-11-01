# Worker job wait and bounded output — Design

**Date:** 2026-11-01   **Status:** Approved   **Author:** Falconiere Barbosa (agent)   **Topic:** `toolu epic job` waits for admission and prints a bounded log tail, Claude workers get long Bash timeouts, and the worker texts stop asking for sleep-and-retry

## Problem

Epic 737's Codex workers made about 550 tool calls per issue; about 46 of them were `sleep` or 20 s `wait` turns, each a full model round trip over a 140k-token cached prefix. Two toolu behaviors cause them. `toolu epic job` refuses admission at once on capacity or pressure (`job-wait` journaled, exit non-zero) and the worker brief says "wait and retry the same check", so the worker polls; admission waits ran p50 4.6 min and p90 17.7 min on 56 of 906 attempts. On a non-zero exit the verb prints only `command exited N` and the merged log stays unseen under the resource root, so workers wrap gates in `> /tmp/gate.txt 2>&1` and tail the file in extra turns. On success it prints the whole stdout. Claude workers also run with Claude Code's 2 min Bash default, below the gate's 108 s average and 344 s p90, which forces background polling there too. Brainstorm: `docs/toolu/brainstorms/2026-11-01-worker-speed-and-token-efficiency.md`.

## Non-Goals

1. No gate policy change (no changed-crates gate mode, no new gate flags).
2. No engine push model: a job does not detach and the engine does not prompt the worker on job completion.
3. No change to the plan ledger's evidence capture or to `toolu ledger` output.
4. No Codex configuration: Codex's `exec_command` yields after at most 30 s and only the brief can shape its polling.
5. No metrics schema change: `toolu epic metrics` keeps reading `admission/job` and `admission/job-wait` records as today.
6. No worktree file writes: the Claude settings file lives in the epic state directory.

## Architecture

Three bounded changes, one PR.

**Admission wait in the shared job runner.** `run_managed_job` (`crates/core/engine/src/resources/jobs.rs`) grows a waiting form. A refusal is retryable only when `acquire_lease` fails with `resource hold:` or `job capacity exhausted`; the runner then sleeps one poll interval and tries again until the bound passes, reporting each attempt and refusal to an observer. The plan ledger's managed check (`crates/core/engine/src/ledger/check.rs`) calls the same runner and so waits with a silent observer; its bound comes from `PLAN_LEDGER_JOB_WAIT` in the check's environment (seconds, GNU `timeout` grammar like the step timeout, default 1800, `0` refuses at once) and its poll is the bound divided by 20, clamped to 50 ms–30 s. That is the only ledger change; evidence capture and output stay as they are (Non-Goal 3). `toolu epic job` (`crates/epic-orchestrator/src/job.rs`) waits with the default bound and journals every attempt as `admission/job` and every refusal as `admission/job-wait`, the same records one `toolu epic job` invocation per retry writes today, so `toolu epic metrics` keeps measuring waits and pressure holds; it also keeps the `waiting-jobs/<key>.json` marker up to date through `waiting_jobs::mark` and clears it on admission. `--no-wait` restores the immediate refusal for scripts and tests. The default bound is 30 min and the poll interval 30 s, the pressure sample age (`PRESSURE_SAMPLE_MS`).

**Bounded log tail.** After the job ends, the verb finds its log through `job_logs::list(root, key)` (the newest entry for the key with this label) and reads the tail with `job_logs::read`. On a non-zero exit the diagnostic is `toolu epic job: command exited N`, the last 80 lines of the merged log, and `full log: <path>`. On success the verb prints stdout whole when it is 40 lines or fewer, else the last 40 lines and `full log: <path>` (Jev: tail_when_long 0.86). The decisive trade-off: fewer input tokens per gate against a worker that pipes a job's stdout, which no first-party text asks for.

**Claude worker timeouts.** The launch writes `<state>/claude-settings.json` with `{"env":{"BASH_DEFAULT_TIMEOUT_MS":"900000","BASH_MAX_TIMEOUT_MS":"5400000"}}` and `agent_args` (`crates/epic-orchestrator/src/hosts.rs`) adds `--settings <that path>` for Claude Code; Claude Code applies a settings file's `env` block to the session and its subprocesses (Jev: 15 min default, 90 min ceiling 0.86). The file is rewritten on every launch so its content follows the binary. Codex, Cursor and Muse get no flag.

**Worker texts.** The worker brief (`plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`) and the delivery-flow execution reference (`plugins/delivery-flow/skills/delivery-flow/references/execution.md`) say: run a gate or test job as one blocking `toolu epic job` call that waits for admission itself; never sleep and retry; on Claude pass a `timeout` up to 90 min for gate jobs; on Codex poll a still-running session with the maximum yield (an empty `write_stdin` poll of 300 s) rather than `sleep` or short `wait` calls; do not redirect job output to a file and tail it, the verb prints the failure tail and the log path; run crate-scoped checks per step and the full gate once before push and once after each fix round. `plugins/epic-orchestrator/README.md` replaces "Admission refusal means wait for capacity and retry the same mandatory check" with the waiting behavior and the flag.

Reused paths: `acquire_lease` refusal messages, `waiting_jobs`, `journal::append`, `job_logs::{list, read}`, the ledger check's `last_lines` precedent, `hosts::agent_args` and its safe-argument regex (a path under the state directory matches it).

## Interfaces / Schema

```rust
// crates/core/engine/src/resources/jobs.rs
/// What the runner tells its caller about admission.
pub enum Admission<'a> { Attempt, Refused(&'a str) }
/// How long to wait for admission, how often to retry, and who hears it.
pub struct JobWait<'a> {
  pub bound: Duration,
  pub poll: Duration,
  pub observe: &'a mut dyn FnMut(Admission<'_>),
}
impl JobWait<'_> {
  /// Refuse at once, telling nobody.
  pub fn none() -> JobWait<'static>;
}
pub const JOB_WAIT_BOUND: Duration = Duration::from_secs(30 * 60);
pub const JOB_WAIT_POLL: Duration = Duration::from_secs(30);
pub fn run_managed_job(argv: &[String], binding: &ResourceBinding, spec: &Spec) -> Result<Output, String>; // == run_managed_job_waiting(.., &mut JobWait::none())
pub fn run_managed_job_waiting(argv: &[String], binding: &ResourceBinding, spec: &Spec, wait: &mut JobWait<'_>) -> Result<Output, String>;
```

A bound that passes returns the last refusal message with the wait appended: `job capacity exhausted (2); waited 1800 s`.

```text
toolu epic job [--no-wait] -- <command> <args...>
  --no-wait   Refuse at once when capacity or pressure holds instead of waiting up to 30 min
```

Verb output. Success, stdout: the command's stdout, or its last 40 lines then `full log: <path>` when longer. Failure, stderr:

```text
toolu epic job: command exited 1
<last 80 lines of the merged log>
full log: /Users/me/.local/state/toolu/resources/jobs/logs/<token>.log
```

Claude settings file, `<state>/claude-settings.json`:

```json
{"env":{"BASH_DEFAULT_TIMEOUT_MS":"900000","BASH_MAX_TIMEOUT_MS":"5400000"}}
```

`agent_args(HostKind::Claude, opts)` appends `--settings <path>` when `opts.settings` is `Some`; `AgentOpts` and `AgentPlan` gain `settings: Option<String>`. The local launch fills it from the state directory; the hc remote launch and the hc review argv leave it `None`, since the file is local. `fixtures/epic/hosts.json` gains a Claude `args` case with `settings` and the fixture test reads `opts["settings"]`.

```rust
// crates/epic-orchestrator/src/job.rs
pub(crate) fn run_job_with(argv: &[String], cwd: &Path, bound: Duration, poll: Duration) -> Outcome; // the verb passes JOB_WAIT_BOUND or ZERO, and JOB_WAIT_POLL
```

Journal records per waiting job: `admission/job` (note: the argv) before each attempt; `admission/job-wait` (note: `capacity` or `pressure`) after each refusal; nothing new.

## Failure modes and edge cases

- **Capacity or pressure refusal:** wait, poll every 30 s, up to 30 min; each cycle journals as today. Past the bound the verb exits non-zero with the last refusal and the seconds waited; the waiting marker stays until the next admission clears it.
- **Other `acquire_lease` errors** (`existing ownership`, invalid state): fail at once, no retry, no `job-wait`.
- **`--no-wait`:** one attempt; refusal exits non-zero exactly as today.
- **Journal write fails during a wait:** the refusal cycle names it in the final message as today (`job-wait not journaled: …`) and the wait continues.
- **Log not found after the run** (retention raced it away, or `list` fails): the verb prints the exit line and `full log: unavailable (<reason>)` and keeps the exit code.
- **Log in ring mode** (over 8 MiB): `read` returns the kept window; the tail still comes from the newest bytes.
- **Output with no trailing newline, CR progress bars, or NUL bytes:** the tail is split on `\n` only; bytes pass through as the log holds them.
- **Settings file cannot be written:** the Claude launch fails with the path and the error before starting the agent, like any other launch failure.
- **Managed settings:** an organization setting overrides the `--settings` env block; the flag never breaks the launch.
- **Timeout of the job itself:** unchanged, 60 min, exit 124.
- **Ledger managed check under pressure:** waits up to `PLAN_LEDGER_JOB_WAIT` (default 1800 s); an unparsable value uses the default; the step's `PLAN_LEDGER_STEP_TIMEOUT` starts when the command starts, not during the wait.

## Acceptance criteria

- **AC-1:** Given a resource root whose only job slot is held by another lease, `run_managed_job_waiting` with a 5 s bound and a 50 ms poll returns the command's output once that lease is released 300 ms later, and its observer saw `Attempt`, at least one `Refused("job capacity exhausted (1)")`, then `Attempt`.
- **AC-2:** Given the same held slot and a 100 ms bound, the runner returns `Err` whose message starts with `job capacity exhausted (1); waited` and no job ran.
- **AC-3:** Given a held slot released 300 ms later, `toolu epic job -- true` run through `run_job_with` with a 5 s bound and a 50 ms poll exits 0 and the journal holds, in order, `job`, `job-wait capacity`, `job` for its key, and no `waiting-jobs/<key>.json` marker remains.
- **AC-4:** Given a held slot, `toolu epic job --no-wait -- true` exits non-zero with `toolu epic job: job capacity exhausted (1)` and the journal holds `job` then `job-wait capacity`, as today.
- **AC-5:** Given `toolu epic job -- sh -c 'seq 1 100; echo boom >&2; exit 3'`, the verb exits non-zero and its stderr holds `toolu epic job: command exited 3`, 80 log lines that include `100` and not `20` (stdout and stderr land in the log in read order, so `boom` may sit at either end), and a `full log: ` line naming an existing file that holds `boom`.
- **AC-6:** Given `toolu epic job -- seq 1 100`, stdout holds lines `61` … `100` and `full log: <path>`, not `1`; given `toolu epic job -- seq 1 40`, stdout holds exactly `1` … `40` and no `full log` line.
- **AC-7:** Given a Claude launch, `<state>/claude-settings.json` exists with the env block above and the agent argv contains `--settings` followed by that path; a Codex argv contains no `--settings`.
- **AC-8:** The rendered worker brief contains "one blocking call", "never sleep and retry", "write_stdin", "full log" and no "wait and retry"; `cargo xtask check-markdown-cli` passes on the brief, the execution reference and the README.
- **AC-10:** Given a bound worktree whose only job slot is held by another lease released 300 ms later and `PLAN_LEDGER_JOB_WAIT=5` in the check environment, the managed ledger check `printf ok` exits 0 with output `ok`; with `PLAN_LEDGER_JOB_WAIT=0` and the slot still held, `run_check` returns `Err` naming `job capacity exhausted (1)`.
- **AC-9:** `cargo xtask docs-cli` regenerates `docs/cli/epic.md` and `docs/cli/commands.json` with `--no-wait`, and `cargo xtask check-cli-compat` passes.

## Acceptance evidence

| AC | Real input | Expected observable result | Boundary or failure case | Check |
|---|---|---|---|---|
| AC-1 | temp resource root, one `occupied` job lease, release thread after 300 ms, `printf ok` | `Ok(output)` with `ok`, observer sequence Attempt, Refused, Attempt | lease released only after the bound: AC-2 | `cargo test -p toolu-engine resources::jobs` |
| AC-2 | same root, bound 100 ms | `Err` starting `job capacity exhausted (1); waited` | — | same |
| AC-3 | temp root bound to a git worktree, held lease released after 300 ms, `toolu epic job -- true` through `run_job` | exit 0; journal `job`, `job-wait capacity`, `job`; no marker | — | `cargo test -p toolu-epic-orchestrator job` |
| AC-4 | same with `--no-wait` | exit failure, message as today, journal `job`, `job-wait capacity` | — | same |
| AC-5 | `sh -c 'seq 1 100; echo boom >&2; exit 3'` | stderr with exit line, 80 tail lines including `100`, `full log: <existing path>` whose file holds `boom` | log missing: `full log: unavailable (…)` | same |
| AC-6 | `seq 1 100` and `seq 1 40` | tail of 40 plus path; whole output and no path | exactly 40 lines prints whole | same |
| AC-7 | `agent_args` with `settings: Some(path)` for Claude and Codex; a launch test that writes the file | `--settings <path>` present only for Claude; file content equals the env block | unwritable state dir fails the launch | `cargo test -p toolu-epic-orchestrator hosts`, `launch` |
| AC-8 | `render_brief` for Claude and Codex; the Markdown files | phrases present and absent as listed | — | `cargo test -p toolu-epic-orchestrator brief`, `cargo xtask check-markdown-cli` |
| AC-10 | bound temp project, held lease released after 300 ms, env `PLAN_LEDGER_JOB_WAIT=5` then `0` | exit 0 and `ok`; `Err` with the refusal | invalid `PLAN_LEDGER_JOB_WAIT` text falls back to the default bound | `cargo test -p toolu-engine --lib ledger::check::tests` |
| AC-9 | the command tree | regenerated docs committed; compat check passes | — | `cargo xtask docs-cli && git diff --exit-code docs/cli`, `cargo xtask check-cli-compat` |

After the next epic run: `toolu epic metrics --epic <key> --json` should show `jobs.waits` near zero and the rollout profile fewer `sleep`/`wait` calls (re-profile per memory `epic-worker-time-profile`).

## Documentation impact

- `plugins/epic-orchestrator/README.md`: the admission paragraph (waiting, `--no-wait`, `PLAN_LEDGER_JOB_WAIT`, bounded output, Claude timeouts).
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`: the "Never lose progress" and pipeline wording.
- `plugins/delivery-flow/skills/delivery-flow/references/execution.md`: gate cadence and job call wording.
- `docs/cli/epic.md` and `docs/cli/commands.json`: generated.
- `docs/resource-budgets.md`: none; `toolu epic job` is not a budgeted hook.

## Open Questions

- Should the ledger's managed check report its waits somewhere visible? Non-blocking; today it reports nothing and the step timeout excludes the wait. Owner: follow-up if metrics show ledger-side waits.
- Codex may still poll once per 5 min during a long gate; if the next profile shows that cost, the engine push model from the brainstorm is the next step. Non-blocking.
