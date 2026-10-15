# Config fixtures

Real `toolu.config.json` files for the config loader tests
(`crates/core/runtime/src/config/tests/`).

- `docs-*.json` — the examples in `docs/config.md`.
- `repo-own-comemory.json` — this repository's own `.claude/toolu.config.json`.
- `edge-*.json` — valid envelopes with invalid or unusual values; the loader
  tests require the values the bash libs resolved.
- `merge-project.json` — the project-side overlay for the user+project merge runs.
- `fail-closed-*.json` — rejected envelopes (unknown top-level key, version 2,
  non-object). The loader fails closed on these by design; bash did not.

The shipped `plugins/toolu/settings/toolu.config.example.json` is read in place.

`expected.json` is the loader golden (#414), captured once from the TypeScript
loader (since removed). Each case places a fixture (`{"file": …}`) or a raw text (`{"text": …}`)
as the user and/or project file on Claude or Codex, and records what loading
yields: the `invalid` reason and loader warnings (absolute paths written as
`$USER_CONFIG` and `$PROJECT_CONFIG`), the merged `data`, and `resolved`, one
entry per resolver call (`gateMode pushReview codex`, `model review`,
`qualityThreshold rust maxFnLines`, `docsSync codeSurfaces`, `enabled hooks
pre-tools`, …) with its value and the warnings that call printed.
`crates/core/runtime/tests/config_fixture.rs` reproduces every case.
