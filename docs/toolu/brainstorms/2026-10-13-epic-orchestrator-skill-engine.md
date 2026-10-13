# Brainstorm: the epic-orchestrator skill, worker brief and commands for the engine (#452)

## Capsule

- **Outcome:** the orchestrator LLM previews, starts, waits for judgments and
  stops; it never runs a gate, merge or launch itself. Workers report
  `babysit` and go idle until the engine prompts them. The skill is under 150
  lines and every `toolu` command in the plugin's Markdown exists.
- **Material defaults / non-goals:** Markdown only. No new `toolu epic` verb;
  the verbs the issue names that the CLI lacks map onto existing ones (below).
  The engine's behaviour does not change.
- **Repository evidence:** `docs/cli/epic.md` (the verb list),
  `crates/epic-orchestrator/src/watch.rs` (`waiting`: a PR is watched from
  `pr-open`, `babysit` and `ready`), `babysit.rs` / `babysit_next.rs` (a green
  tick sets `ready` and queues the merge; a finding prompts the worker with
  `pr-babysit: <repo>#<n> needs you (...)`), `query.rs` (`wait_default`: 2700 s
  on Claude Code, 480 s elsewhere), `crates/cli/tests/epic_markdown.rs` and
  `tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts` (the existing
  skill contracts).
- **Risk:** the OpenCode generated tree must not carry Claude-only wording
  (`run_in_background`, "end your turn"); the port map needs a new anchor. The
  live two-issue sandbox epic needs a GitHub sandbox repository that this
  worker is not authorized to create.
- **Handoff:** spec.

## Axes

**Interface (the CLI the Markdown names).** The issue lists `toolu epic
preflight`, `toolu epic stop`, a `relaunch` judgment command and `toolu epic
start <epic> [--max N --hosts …]`. None exists: `start` takes a state
directory. Jev (choice, 0.99 markdown_only over 0.01 preflight_only and 0.00
all_verbs) agreed with the issue's Goal ("Rewrite epic-orchestrator's
Markdown"), and the drift gate fails on any command the CLI lacks. Mapping:

| Issue names | Markdown runs |
|---|---|
| `toolu epic preflight` | `toolu doctor`, then a short checklist (herdr pane and server, `gh auth status`, each host on `PATH`, `gh api rate_limit`, tracker env) |
| `toolu epic start <epic> --max --hosts` | `toolu epic graph <epic> --max N --save`, `toolu epic route --graph … --hosts …`, `toolu epic start <state_dir>` |
| `relaunch` | `toolu epic launch --graph … --issue … --reprompt` or `--replace` |
| `toolu epic stop` | `toolu epic pause <epic-key>`, then end the wait loop; `toolu epic finish … --abandon` per issue only when the user asks |

**Waiting per host.** Claude Code: `toolu --json epic wait` in a background
Bash call (its 2700 s default), and the turn ends; the call's completion
re-invokes the orchestrator. Codex and OpenCode: a foreground loop of
`toolu --json epic wait --max-seconds 480`, because one tool call cannot block
for hours. The OpenCode port replaces the Claude paragraph.

**Worker brief.** After `report babysit --pr <n>` the worker stops: no
`pr-babysit` cron and no polling. The engine's ticks prompt it with a tick
result when something needs fixing (the prompt names the `toolu babysit`
commands to act with), `REBASE` only on a conflict, and `STATUS?`. A green tick
moves the issue to `ready` itself, so the worker no longer reports `ready`.

**Size.** The skill drops the engine's internal description (batching,
coupling, snapshots, merge-queue mechanics) and the manual gate, which the
engine runs; recovery details move to `references/recovery.md`. The skill's
description is added to the context-budget gate.

## Rejected

- **Add `toolu epic preflight` only:** deterministic and useful, but it needs a
  host-plugin probe that `crates/plugins` owns and plugin crates may not share;
  a separate CLI issue can add it later.
- **Add every missing verb:** a Rust change across the CLI contract in a
  Markdown issue, with `HOOK_PROTOCOL` cost if any is later renamed.
- **Keep the manual merge gate in the skill:** the engine merges; the gate
  stays documented in `docs/cli/epic.md` and `references/recovery.md` for a
  repair by hand.
