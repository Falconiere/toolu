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
