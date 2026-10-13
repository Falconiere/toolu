# epic-orchestrator evals (#452, AC-10)

Date: 2026-10-13. Each eval ran in a fresh subagent (Sonnet) that read the
shipped `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md` and
answered the prompt from `evals/evals.json` (the stop prompt names the test
epic `demo-1` instead of `falconiere-toolu-402`). The read-only evals ran this
branch's `target/debug/toolu` against GitHub; "where is the epic at" and
"stop" ran against the isolated engine of
`2026-10-13-epic-wait-hosts.md` (`/tmp/w452/iso.sh`). Grades are mine against
each assertion, confirmed by Jev (noul that every assertion holds: dry-run
0.71, parallel 0.77, where 0.79, stop 0.85).

The real engine's state was the same before and after:
`{"engine":"down","epics":[]}`; `~/.claude/epics/falconiere-toolu-402/graph.json`
was last written before the evals started.

## dry-run-the-epic: PASS

Commands: `toolu doctor`, `gh auth status`, `gh api rate_limit`,
`herdr status`, `toolu --json epic graph Falconiere/toolu#402 --max 3
--dry-run > /tmp/epic-graph.json`, `toolu epic route --graph
/tmp/epic-graph.json --hosts claude:2,codex:1 --dry-run`, `jq` reads.

- graph `--dry-run`, never `--save`: met.
- route `--dry-run`: met.
- ready issues and launch batch, blocked issues with blockers: met (52 done;
  #425, #451, #452 in flight; #453 ready; #437, #462, #440 blocked with their
  blockers; the batch is empty because three workers fill `--max 3`).
- host, model and effort per batch issue: met vacuously; the batch was empty
  and the answer said the route table had no rows.
- no start, launch or herdr write, no state: met.
- says the dry run stops: met ("Nothing was started, saved or written").

## which-issues-can-run-in-parallel: PASS

Same read-only commands. Treated as a dry run; named #453 as the only ready
issue, the empty batch and why, and the blockers of #437, #462 and #440;
launched and wrote nothing.

## where-is-the-epic-at: PASS

Commands: `toolu epic status demo-1`, `toolu epic queue demo-1`. Summarized
each issue's stage, phase and PR, the open judgments and the empty queue, and
gave `http://127.0.0.1:7717/`. No `wait`, `ack` or `answer`; nothing changed.

## stop: PASS

Commands: `toolu epic pause demo-1`, `toolu epic status demo-1` (now
`paused: true`). Said launches, rebases and merges stop and workers go idle,
named `toolu epic resume demo-1`, tore nothing down and started no wait.

## Observed, outside this change

Both read-only runs found `toolu doctor` red on `skew`: the installed plugin
cache (`~/.claude/plugins/cache/toolu/*/7.11.0`) has no integer
`hookProtocol`. The skill says to stop there; the eval prompt told the agents
to continue for the dry run.

## work-epic: NOT RUN

It starts workers and merges PRs; it needs the sandbox epic of the spec's Open
Question 1.
