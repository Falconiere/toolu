# Config fixtures

Real `toolu.config.json` files for `@toolu/core/config` tests
(`packages/toolu-core/src/config/__tests__/`).

- `docs-*.json` — the examples in `docs/config.md`.
- `repo-own-comemory.json` — this repository's own `.claude/toolu.config.json`.
- `edge-*.json` — valid envelopes with invalid or unusual values; the parity
  test requires TypeScript and bash to resolve them identically.
- `merge-project.json` — the project-side overlay for the user+project merge runs.
- `fail-closed-*.json` — rejected envelopes (unknown top-level key, version 2,
  non-object). TypeScript fails closed on these by design; bash does not.

The shipped `plugins/toolu/settings/toolu.config.example.json` is read in place.

`expected.json` is the loader golden (#414), captured once from the TypeScript
loader. Each case places a fixture (`{"file": …}`) or a raw text (`{"text": …}`)
as the user and/or project file on Claude or Codex, and records what loading
yields: the `invalid` reason and loader warnings (absolute paths written as
`$USER_CONFIG` and `$PROJECT_CONFIG`), the merged `data`, and `resolved`, one
entry per resolver call (`gateMode pushReview codex`, `model review`,
`qualityThreshold rust maxFnLines`, `docsSync codeSurfaces`, `enabled hooks
pre-tools`, …) with its value and the warnings that call printed.
`packages/toolu-core/src/config/__tests__/config-fixture.test.ts` and
`crates/core/runtime/tests/config_fixture.rs` reproduce every case.
