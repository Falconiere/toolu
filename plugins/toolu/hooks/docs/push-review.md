# push-review hook

PreToolUse hook on `Bash(git push)`. Gates pushes on a clean code review recorded in `<target repo root>/.claude/tmp/push-review/<branch-slug>.json`.

**How firmly it gates is a mode**, not a constant — see [gates.md](./gates.md).
At the shipped default (`balanced`) this hook **advises**: every failed check
below is reported to the agent and the push is not stopped.
`{"gates":{"pushReview":{"mode":"ask"}}}` prompts, and answering yes records
a waiver for that exact diff so the same code is never queried twice.
`block` restores the original hard deny; `off` silences it. On Codex, where
the host cannot prompt, `ask` degrades to `advise`.

## Waivers

1. In `ask` mode the gate writes `<branch-slug>.pending-waiver.json` naming the
   diff SHA it asked about.
2. `post-tools/modules/push-waiver.sh` promotes that marker to
   `<branch-slug>.waiver.json` once the push has actually run **and succeeded**
   — PostToolUse fires when a tool ran, not when it worked, and a rejected
   non-fast-forward push has been reviewed by nobody.
3. A matching waiver makes the gate pass silently (telemetry
   `push_check reason_code=waived`).
4. A new commit changes the diff SHA, the waiver stops applying, and the gate
   asks again.

A refused prompt leaves the pending marker orphaned; the SessionStart state
sweeper reclaims it.

Detection lives in `lib/detect.sh:is_git_push`, which is **structural, not textual**: the command is split into statements (honouring shell quoting), each statement is tokenized, and git's real subcommand is resolved after consuming git's global options. So `git -C <path> push`, `git -c k=v push`, `git --no-pager push`, and `git --git-dir=<path> push` are all gated, while `git commit -m "push"` is a commit.

Prose is not a command. `echo "no git push rules remain"`, a grep pattern like `grep -qE "^git push" file`, or a commit message that explains the gate are all arguments, not invocations, and do not fire it — the earlier regex over the raw string did fire on every one of them. Heredoc bodies are stripped first, and `$(...)` / backtick bodies are scanned as commands in their own right, so `out=$(git push)` is still a push.

## Flow

1. Agent runs `git push`.
2. Hook resolves the **target repo root** with `push_target_root`: every `-C <path>` preceding `push` in that command segment, replayed cumulatively the way git itself applies them (a `-C` belonging to another command in a `&&` chain is ignored), else the cwd's `git rev-parse --show-toplevel`, else `$CLAUDE_PROJECT_DIR`, else the cwd. Every git read below runs against that root, not the hook's cwd — a `git -C <worktree> push` is judged on the worktree's own branch, diff, and state file.
3. Hook computes `git diff <base>...HEAD | git hash-object --stdin` in the target repo, where `<base>` is resolved dynamically via `detect_base_branch <root>` (origin/HEAD, falling back to `main`; `$PUSH_REVIEW_BASE` overrides for tests).
4. Hook reads `<target repo root>/.claude/tmp/push-review/<branch-slug>.json`. `$STATE_DIR` overrides the directory (tests, and the `toolu-review` state writer honours the same variable).
5. If the state file is missing, has a stale `diff_sha`, has `findings_count > 0`, or is schema `version: 1` → DENY with instructions.
6. Agent runs a reviewer against the diff and applies its findings. The gate is **reviewer-agnostic** — it accepts at least one of: \`caveman:cavecrew-reviewer\`, \`code-review\`, \`toolu-review:review\`, \`code-review:xhigh\`, \`review\`, \`security-review\`. Prefer \`caveman:cavecrew-reviewer\` when the caveman plugin is installed; otherwise use the built-in \`/code-review xhigh --fix\` skill (always available, no plugin required) or the \`toolu-review:review\` skill from the \`toolu-review\` plugin. Running extra reviewers (e.g. `code-simplifier` first) is allowed — the check is membership, not equality.
7. Re-run the reviewer on the new diff and loop until it returns zero findings.
8. Agent writes state file atomically (`<file>.tmp` then `mv`) with `findings_count: 0`, the new SHA, and `reviewed_files` set to every path the reviewer actually covered.
9. Agent retries `git push` → hook allows.

## State schema (version 2)

```json
{
  "version": 2,
  "branch": "feat/x",
  "diff_sha": "<git-hash-object output>",
  "base_branch": "main",
  "reviewed_at": "<iso8601>",
  "reviewers": ["code-review"],
  "findings_count": 0,
  "findings": [],
  "review_round": 1,
  "reviewed_files": ["path/a.ts", "path/b.rs"]
}
```

`version` must normalize to `"2"` (emitted as a JSON number; the gate compares
the `jq -r`-normalized string, so a literal `"2"` string also passes). A
`version: 1` state — the pre-`reviewed_files` schema — gets a dedicated
one-time upgrade deny: *"push-review state is schema v1; harness v2 requires
reviewed_files — re-run the review to regenerate the state file"*. There is no
dual-accept: a v1 state cannot satisfy the v2 gate, because it silently waives
file coverage for whatever the v1 writer claims (see reviewer honesty note
below).

`reviewed_files` (sorted, unique) must equal sorted `git diff --name-only
<base>...HEAD` — the paths the reviewer actually covered. A mismatch denies,
naming missing paths (changed but not reviewed) and extra paths (reviewed but
not in the current diff) separately. This catches an *honestly-reported*
partial review scope; it is not adversary-proof against a reviewer that lies
about what it covered — that's a known, accepted limitation (mechanizing
coverage claims, not reviewer honesty).

`review_round` counts state-file rewrites **at the same `diff_sha`**: it starts
at 1 for a new `diff_sha` and bumps by 1 only when the diff is unchanged. A
changed diff restarts it at 1 — the earlier rounds judged different code, and
nothing resets the counter after a push, so carrying it forward made the cap
terminal for any long-lived branch. The hook treats a missing field as round 1
for backward compatibility and denies with an escalation message once the round
exceeds 5 (`MAX_ROUNDS`), so a fix→re-review loop on an unchanged diff cannot
run unbounded.

The canonical writer, `plugins/toolu-review/skills/review/scripts/write-state.sh`,
emits this v2 schema and auto-computes `reviewed_files` from `git diff
--name-only` (override with `--reviewed-files <comma-list>` for a partial or
adjusted review scope).

## Security posture

`security-review` is **not separately enforced** by this gate (dropped in v1.2.0 per project decision) — though it is one of the accepted reviewers, so running it satisfies the gate. For diffs that touch authentication, secret handling, request parsing, or other security-sensitive code, run `/security-review` before push. The gate's reviewer catches correctness and clarity bugs but makes no security guarantees on its own.

## Failed checks

Each of these fails the gate. What the user sees — a denial, a prompt, a note,
or nothing — is the mode's business, not this list's.

- State file missing.
- State file SHA != current diff SHA (diff changed).
- `findings_count > 0`.
- `version: 1` state file — one-time schema-upgrade deny (re-run the review to regenerate it).
- Corrupted JSON or schema drift (wrong `version`, missing `diff_sha`/`findings_count`).
- `reviewers` contains no accepted reviewer.
- `reviewed_files` does not match the diff's changed paths (missing and/or extra paths named).
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
