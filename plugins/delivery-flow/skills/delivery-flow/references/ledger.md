# Plan ledger reference

The JSON array under `## Steps (machine-readable)` is the authoritative detailed
step list. Each step requires non-empty string `id`, `title`, and `check`.

Optional fields preserve the existing ledger contract:

- `ac_refs`: spec `AC-<n>` ids covered by this step.
- `depends_on`: earlier step ids that must be green before this work.
- `paths`: pathspecs read by the step's check, including its test file.
- `input`: the real input or fixture used to prove the behavior.
- `model`: `haiku`, `sonnet`, `opus`, `fable`, or `inherit`.

`paths` narrows iteration freshness to what the check actually reads. Declare
all such paths: under-declaring can leave a stale green result. Before delivery,
`toolu ledger run <plan_doc> --verify` re-runs steps without paths and steps
whose paths changed. Give the final full-gate step no `paths`, so one check
always runs on the final diff; when every step declares paths, `--verify`
re-runs them all.

Add `--detach` to return a job id while a native copy runs the same checks. Relative
plan paths resolve from the launch directory, and checks in a bound worktree
still acquire job capacity automatically. Enabled post-tool or prompt hooks in
a bound worktree report a finished result once. Read the saved result described
in the [toolu plugin guide](../../../../toolu/README.md); launch success only confirms
that the copy started. Before writing the ledger, the copy compares the working
tree with its starting contents, including untracked files that Git does not
ignore. A changed tree produces `stale: true` and `exit_code: 2`, preserving the
prior ledger even with `--step`; rerun against the stable tree.
