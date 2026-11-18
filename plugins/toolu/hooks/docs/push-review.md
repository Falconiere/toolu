# Push review

The push-review gate checks the target repository's branch diff against review
state in the host's project state directory, under `push-review/`
(`$STATE_DIR` overrides the review directory). On Qwen Code the default is
`.qwen/tmp/push-review/` in the target repository. The balanced preset uses
`advise`, `block` denies, `ask` requests approval, and `off` is silent.
On Codex, `ask` degrades to `advise`. Qwen Code asks for each push that
requires approval. See [gates.md](gates.md)
for modes and waiver behavior.

In Qwen Code, run `/toolu-review:review` when the gate requests review.
On a supported native review host, run `code-review:review` on the committed
diff. It falls back to `toolu-review:review` when necessary. Fix findings and rerun review after changes.
The native `toolu review write-state` writer records version 2 state; do not
fabricate review coverage. State must match the current diff SHA, contain zero
findings, an accepted reviewer, and exactly the changed paths in `reviewed_files`.
Version 1, missing, corrupt, stale or incomplete state fails the gate. A review
round above five on the same diff, unresolved target branch, missing base or
empty diff also fails. For detached review, preserve the target branch with
the writer's `--branch` and the pushed repository with `--repo`.
The writer defaults to the changed paths, spelled as
`git -c core.quotepath=true diff --name-only` prints them (non-ASCII names
C-quoted) whatever the user's `core.quotepath`; the push gate and the
`toolu ledger verdict status` review gate compare that same spelling. Use
`--reviewed-files` only to record the paths actually reviewed.

Push detection parses shell commands, including git global options and command
substitutions; prose mentioning a push does not trigger it. Static `git -C`
targets are resolved in the target repository. Security review is not separately
enforced: assess security-sensitive changes during review.

The native contracts and focused tests live in
`crates/core/engine/src/gates/tests/push_review_test.rs`,
`crates/core/engine/src/gates/tests/push_target_test.rs`, and
`crates/cli/tests/golden_gates.rs`.
