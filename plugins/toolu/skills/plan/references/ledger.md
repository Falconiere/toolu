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
`plan-ledger --verify` judges every step against the whole branch diff, so
scoped freshness never replaces final verification.
