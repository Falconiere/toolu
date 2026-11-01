# Brainstorm: worker speed and token efficiency

**Date:** 2026-11-01   **Mode:** Standalone, Full path   **Topic:** make epic workers finish an issue in fewer turns and tokens

## Capsule

- **Outcome:** an epic worker spends no agent turns polling for gate jobs or admission, reads gate failures from the job verb's own output, and runs one blocking call per expensive job. Measured by `toolu epic metrics` (`jobs.waits`, `pressure_holds`) and by the per-issue call count in host rollouts, against the 2026-11-01 profile (median 550 calls, about 46 idle `sleep`/`wait` turns, 5 gate jobs per issue).
- **Material defaults:**
  1. `toolu epic job` waits for admission itself, bounded (default 30 min), journaling each refusal cycle as today's `job-wait`; a `--no-wait` flag keeps the immediate refusal for scripts and tests.
  2. `toolu epic job` prints a bounded tail of the captured log on a non-zero exit (last 80 lines plus the log path) and bounds success output the same way (last 40 lines plus the path) instead of the whole stdout.
  3. Claude workers launch with `BASH_DEFAULT_TIMEOUT_MS=900000` and `BASH_MAX_TIMEOUT_MS=3600000`; the plan decides whether that lands through the worktree's `.claude/settings.local.json` `env` block or the herdr pane environment.
  4. The worker brief and the delivery-flow execution reference say: one blocking `toolu epic job` call per gate, never sleep-and-retry; on Codex, poll a still-running session with the maximum yield (`write_stdin` empty poll, 300 s) rather than `sleep` or 20 s `wait`; do not redirect gate output to a file and tail it; crate-scoped checks per step, the full gate once before push and once after each fix round.
- **Non-goals:** a changed-crates gate mode or any gate policy change (Jev 1.08 of 3: a few percent, and it needs its own `chore(gates)` PR); the engine push model where a detached job prompts the worker on completion (Jev 0.00); fast-mode or effort routing changes (output is about 110 tokens per turn); SessionStart context trimming (unmeasured, small against a 140k prefix); the post-code merge and answer path (lead-time work, separate epic); compaction guidance for the agent's own reads.
- **Repository evidence:** `crates/epic-orchestrator/src/job.rs` refuses on `acquire_lease` errors (`resource hold:`, `job capacity exhausted`), marks the worktree waiting and exits non-zero; on a non-zero exit it prints only `command exited N` while the merged log sits under `~/.local/state/toolu/resources/jobs/logs/`; the worker brief (`plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`) tells workers to "wait and retry the same check"; `hosts.rs::agent_args` sets no environment; recent plans already scope checks (136 `cargo test -p` against 16 `cargo xtask gate` across 92 plans). Codex `exec_command` yields after at most 30 s (clamp 250–30000 ms) and empty `write_stdin` polls wait 5–300 s; Claude Code Bash defaults to 120 s with a 600 s ceiling, both raised by the two env vars.
- **Risk:** without the Claude env change the blocking job still times out at 2 min and the polling returns; on Codex the 30 s yield clamp leaves one poll per 5 min at best. A worker silent for a long job wait must not read as a stall: `waiting_jobs` already surfaces in `toolu epic status`, and the brief must say job waits are expected. The bounded tail can cut a long failure list; the path covers the rest. `check-cli-compat` and the contract snapshot change for the new flag.
- **Handoff:** `/delivery-flow:delivery-flow` for one PR covering the job verb, the launch environment, the brief and the execution reference; re-run the profile after the next epic and compare `jobs.waits`, `pressure_holds` and the rollout call count.

## Evidence

Research session 2026-11-01 (memory `epic-worker-time-profile`): epic 737 Codex workers, 13 main sessions.

| Measure | Value |
|---|---|
| Session per issue | 74–379 min, median about 150 |
| Tool calls per issue | 270–1,200, median about 550, about 16 s each |
| Context per turn | p50 140k tokens; output p50 110; uncached input p50 280 |
| Idle turns | 362 `sleep` (20 s) + 237 `wait` (20 s yield) = about 46 per issue |
| Gate jobs | about 5 per issue; avg 108 s, p90 344 s, max 840 s |
| Admission | 906 attempts, 189 pressure holds, 56 waits p50 4.6 min, p90 17.7 min |
| Bookkeeping calls | `toolu ledger` about 45, `epic job` about 56, `jev` about 12, `report` about 10 |

External: TraceLab (433K real tool calls) puts tools at 60% of step time with the 4% of calls over one minute making 85% of tool time; Anthropic's programmatic tool calling cut tokens 37%; Unblocked cut calls 64% for a 27% time cut. Jev ranking from the research (0–3): fewer turns 2.59, gate discipline 2.02, ceremony 1.84, lower effort 1.14, admission 1.03, context trim 0.96, fast tier 0.70.

## Axes

- **Interface:** one new flag (`--no-wait`) and bounded output on an existing verb; the brief and a private delivery-flow reference change wording. No hook protocol change. Default: additive, no removal.
- **Behavior:** a job call can now take up to the wait bound plus the command's 3600 s timeout. Default: journal every refusal cycle so metrics keep counting waits; heartbeat the waiting state so status shows it.
- **Persistence:** none new; `waiting-jobs/<key>.json` and the journal already exist.
- **Cost:** fewer turns means fewer cached-prefix reads (about 140k tokens per turn) and fewer output tokens; no new external calls.
- **Reversibility:** `--no-wait` restores today's behavior per call; the env and brief changes are text.
- **Security:** none; the job still runs inside the worktree's lease.

## Alternatives compared (Jev, 2026-11-01)

| Alternative | Probability | Why |
|---|---|---|
| B: blocking job + env + brief | 0.98 | Chosen. One verb change removes the retry loop at its root; works without the engine up. |
| A: brief and env only | 0.02 | Rejected: admission refusals still need agent retries; Codex still polls. |
| C: detached job, engine prompts on completion | 0.00 | Rejected for now: zero polling but a new lifecycle state, engine dependency and larger blast radius. Revisit if Codex's 30 s yield clamp keeps polling turns high after B. |

Bounded failure tail removes the tail-the-log turns without weakening a gate: Jev noul 0.76. Gate policy change now: score 1.08 (a few percent), deferred.

## Open risks

- The gain on Claude depends on the env landing in the worker's session; verify with a real worker launch that `BASH_DEFAULT_TIMEOUT_MS` is visible to the Claude process.
- Pressure recovery needs two calm minutes; a 30 min bound can still expire on a loaded machine. The job then exits non-zero with the wait reason, as today.
- Rollout call counts are host-specific; the Codex profile script lives in the research session, not in the repo.
