# push-review hook

PreToolUse hook on `Bash(git push)`. Blocks pushes until a clean code review is recorded in `<target repo root>/.claude/tmp/push-review/<branch-slug>.json`.

Detection lives in `lib/detect.sh:is_git_push`, which accepts git's global options between `git` and the subcommand — `git -C <path> push`, `git -c k=v push`, `git --no-pager push`, `git --git-dir=<path> push` are all gated. Only tokens starting with `-` are consumed, so `git commit -m "push"` still does not match.

## Flow

1. Agent runs `git push`.
2. Hook resolves the **target repo root** with `push_target_root`: every `-C <path>` preceding `push` in that command segment, replayed cumulatively the way git itself applies them (a `-C` belonging to another command in a `&&` chain is ignored), else the cwd's `git rev-parse --show-toplevel`, else `$CLAUDE_PROJECT_DIR`, else the cwd. Every git read below runs against that root, not the hook's cwd — a `git -C <worktree> push` is judged on the worktree's own branch, diff, and state file.
3. Hook computes `git diff <base>...HEAD | git hash-object --stdin` in the target repo, where `<base>` is resolved dynamically via `detect_base_branch <root>` (origin/HEAD, falling back to `main`; `$PUSH_REVIEW_BASE` overrides for tests).
4. Hook reads `<target repo root>/.claude/tmp/push-review/<branch-slug>.json`. `$STATE_DIR` overrides the directory (tests, and the `toolu-review` state writer honours the same variable).
5. If the state file is missing, has a stale `diff_sha`, or has `findings_count > 0` → DENY with instructions.
6. Agent runs a reviewer against the diff and applies its findings. The gate is **reviewer-agnostic** — it accepts at least one of: \`caveman:cavecrew-reviewer\`, \`code-review\`, \`toolu-review:review\`, \`code-review:xhigh\`, \`review\`, \`security-review\`. Prefer \`caveman:cavecrew-reviewer\` when the caveman plugin is installed; otherwise use the built-in \`/code-review xhigh --fix\` skill (always available, no plugin required) or the \`toolu-review:review\` skill from the \`toolu-review\` plugin. Running extra reviewers (e.g. `code-simplifier` first) is allowed — the check is membership, not equality.
7. Re-run the reviewer on the new diff and loop until it returns zero findings.
8. Agent writes state file atomically (`<file>.tmp` then `mv`) with `findings_count: 0` and the new SHA.
9. Agent retries `git push` → hook allows.

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

`review_round` counts state-file rewrites **at the same `diff_sha`**: it starts
at 1 for a new `diff_sha` and bumps by 1 only when the diff is unchanged. A
changed diff restarts it at 1 — the earlier rounds judged different code, and
nothing resets the counter after a push, so carrying it forward made the cap
terminal for any long-lived branch. The hook treats a missing field as round 1
for backward compatibility and denies with an escalation message once the round
exceeds 5 (`MAX_ROUNDS`), so a fix→re-review loop on an unchanged diff cannot
run unbounded.

## Security posture

`security-review` is **not separately enforced** by this gate (dropped in v1.2.0 per project decision) — though it is one of the accepted reviewers, so running it satisfies the gate. For diffs that touch authentication, secret handling, request parsing, or other security-sensitive code, run `/security-review` before push. The gate's reviewer catches correctness and clarity bugs but makes no security guarantees on its own.

## Failure modes the hook denies

- State file missing.
- State file SHA != current diff SHA (diff changed).
- `findings_count > 0`.
- Corrupted JSON or schema drift (wrong `version`, missing `diff_sha`/`findings_count`).
- `reviewers` contains no accepted reviewer.
- `review_round` exceeds the max (5) on an unchanged diff — escalation deny.
- Detected base branch not present locally.
- Detached HEAD.
- Empty diff against base (no-op push or force-reset branch).

## Tests

`hooks/pre-tools/modules/__tests__/push-review.bats` covers the gate;
`hooks/lib/__tests__/detect.bats` covers `is_git_push` and `push_target_root`.
Run from the repo root with:

```bash
bats plugins/toolu/hooks/pre-tools/modules/__tests__/push-review.bats
bats plugins/toolu/hooks/lib/__tests__/detect.bats
```
