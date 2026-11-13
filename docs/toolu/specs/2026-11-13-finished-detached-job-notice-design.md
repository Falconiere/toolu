# Finished detached job notice — Design

**Date:** 2026-11-13   **Status:** Approved   **Author:** Codex   **Topic:** Issue #878

## Problem

An agent that starts a detached job needs its result without polling. The completed result already exists under the worktree's resource binding, but the agent's next tool or prompt hook does not deliver it.

## Non-Goals

1. New hook entries, polling, or changes to the detached job runner.
2. Notifications for another worktree or for a result already claimed by the engine or another hook.

## Architecture

Add a PostToolUse built-in gate for shell and edit events and a prompt hint. Both use the existing `resource_binding`, `claim_finished`, and `line` APIs. A small shared result helper resolves the current worktree's binding, claims finished records, and formats each line, followed by recorded output for nonzero exits. The existing host encoder carries the post-tool advisory. The prompt appends the text to `additionalContext` during normal hint handling.

## Interfaces / Schema

`job_results::take_notice(cwd: &Path) -> Result<Option<String>, String>` returns no text for an unbound worktree or no finished records. Its text is `job <id> finished: exit <code>[, stale]: <label>` per result, followed by `output` for nonzero exits. No result schema or hook manifest changes.

## Failure modes and edge cases

The binding and claim match the current Git worktree, so another worktree's result stays pending. Concurrent hooks can claim a result only once under the resource lock. Waiting and running records stay pending. An unrelated tool event does not claim a result. A claim error leaves the result available for a later attempt; the post-tool dispatcher reports its normal gate warning, while the prompt keeps its existing hint behavior. Empty, trivial, and blocked prompts do not claim a result because they cannot deliver a normal hint.

## Acceptance criteria

- **AC-1:** Given one finished result for the current worktree, the next post-tool shell or edit call emits its line once and a second call emits nothing.
- **AC-2:** Given a result bound to another worktree, a post-tool call emits nothing and leaves that result unclaimed.
- **AC-3:** Given a finished result and a normal prompt, `additionalContext` contains the result line and a later hook does not repeat it.
- **AC-4:** Given a failed finished result, the notice includes its recorded output after the line.
- **AC-5:** The existing post-tool and prompt hook latency benchmarks remain within their asserted budgets.

## Acceptance evidence

- AC-1 and AC-2: `cargo test -p toolu-cli --test tool_hooks` runs the real binary against two temporary Git worktrees and the actual result files, checking host JSON and `delivered_at`.
- AC-3 and AC-4: `cargo test -p toolu-hub --lib -- prompt_submit` uses real temporary binding and result files, checking `additionalContext`, one-time claim, and failed output.
- AC-5: `cargo xtask bench-hooks --only toolu/post-tools --only toolu/user-prompt-submit --assert` checks both real hooks.

## Documentation impact

Update `docs/cli/epic.md` to explain that the next tool call or prompt delivers a finished detached job once.

## Open Questions

None.
