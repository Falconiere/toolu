# push-review hook

PreToolUse hook on `Bash(git push)`. Blocks pushes until a clean code review is recorded in `.claude/tmp/push-review/<branch-slug>.json`.

## Flow

1. Agent runs `git push`.
2. Hook computes `git diff <base>...HEAD | git hash-object --stdin`, where `<base>` is resolved dynamically via `detect_base_branch` (origin/HEAD, falling back to `main`; `$PUSH_REVIEW_BASE` overrides for tests).
3. Hook reads `.claude/tmp/push-review/<branch-slug>.json`.
4. If the state file is missing, has a stale `diff_sha`, or has `findings_count > 0` → DENY with instructions.
5. Agent runs a reviewer against the diff and applies its findings. The gate is **reviewer-agnostic** — it accepts at least one of: \`caveman:cavecrew-reviewer\`, \`code-review\`, \`toolu-review:review\`, \`code-review:xhigh\`, \`review\`, \`security-review\`. Prefer \`caveman:cavecrew-reviewer\` when the caveman plugin is installed; otherwise use the built-in \`/code-review xhigh --fix\` skill (always available, no plugin required) or the \`toolu-review:review\` skill from the \`toolu-review\` plugin. Running extra reviewers (e.g. `code-simplifier` first) is allowed — the check is membership, not equality.
6. Re-run the reviewer on the new diff and loop until it returns zero findings.
7. Agent writes state file atomically (`<file>.tmp` then `mv`) with `findings_count: 0` and the new SHA.
8. Agent retries `git push` → hook allows.

## State schema

```json
{
  "version": 1,
  "branch": "feat/x",
  "diff_sha": "<git-hash-object output>",
  "base_branch": "main",
  "reviewed_at": "<iso8601>",
  "reviewers": ["code-review"],
  "findings_count": 0,
  "findings": [],
  "review_round": 1
}
```

`review_round` starts at 1 and must bump by 1 on every rewrite of the state
file (each fix→re-review loop). The hook treats a missing field as round 1
for backward compatibility and denies with an escalation message once the
round exceeds 5 (`MAX_ROUNDS`), so the loop cannot run unbounded.

## Security posture

`security-review` is **not separately enforced** by this gate (dropped in v1.2.0 per project decision) — though it is one of the accepted reviewers, so running it satisfies the gate. For diffs that touch authentication, secret handling, request parsing, or other security-sensitive code, run `/security-review` before push. The gate's reviewer catches correctness and clarity bugs but makes no security guarantees on its own.

## Failure modes the hook denies

- State file missing.
- State file SHA != current diff SHA (diff changed).
- `findings_count > 0`.
- Corrupted JSON or schema drift (wrong `version`, missing `diff_sha`/`findings_count`).
- `reviewers` contains no accepted reviewer.
- `review_round` exceeds the max (5) — escalation deny.
- Detected base branch not present locally.
- Detached HEAD.
- Empty diff against base (no-op push or force-reset branch).

## Tests

`hooks/pre-tools/modules/__tests__/push-review.bats` — run from the repo root with:

```bash
bats plugins/toolu/hooks/pre-tools/modules/__tests__/push-review.bats
```
