# Push review

The push-review gate checks the target repository's branch diff against review
state in the host's `.claude/tmp/push-review/` or `.codex/tmp/push-review/`
directory (`$STATE_DIR` overrides it). It follows the configured
mode: balanced defaults to `advise`; `block` denies, `ask` requests a waiver,
and `off` is silent. On Codex, `ask` degrades to `advise`. See [gates.md](gates.md)
for modes and waiver behavior.

Run `code-review:review` on the committed diff; it falls back to
`toolu-review:review` when necessary. Fix findings and rerun review after changes.
The native `toolu review write-state` writer records version 2 state; do not
fabricate review coverage. State must match the current diff SHA, contain zero
findings, an accepted reviewer, and exactly the changed paths in `reviewed_files`.
Version 1, missing, corrupt, stale or incomplete state fails the gate. A review
round above five on the same diff, unresolved target branch, missing base or
empty diff also fails. For detached review, preserve the target branch with
the writer's `--branch` and the pushed repository with `--repo`.
The writer defaults to the changed paths; use `--reviewed-files` only to
record the paths actually reviewed.

Push detection parses shell commands, including git global options and command
substitutions; prose mentioning a push does not trigger it. Static `git -C`
targets are resolved in the target repository. Security review is not separately
enforced: assess security-sensitive changes during review.

The native contracts and focused tests live in
`crates/core/engine/src/gates/tests/push_review_test.rs`,
`crates/core/engine/src/gates/tests/push_target_test.rs`, and
`crates/cli/tests/golden_gates.rs`.
