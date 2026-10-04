# OpenCode independently loadable npm package (#361, OP-27) — Brainstorm

- **Outcome:** The `@toolu/opencode` tarball comes from the real publish path: `npm pack`, which runs the package's `prepack`. It installs into a clean OpenCode sandbox that has no `TOOLU_REPO_ROOT` or `TOOLU_ROOT` and no checkout. It loads all 16 plugins through the pinned host. Every shipped helper, surface reference and module import resolves inside the installed tarball, or comes from a declared registry dependency.
- **Material defaults/non-goal:** `@toolu/core` keeps a caret range, and release-please now raises its floor at every release. `@opencode-ai/plugin` and `@opencode-ai/sdk` become optional peers pinned to the contract's SDK version. Test files leave the tarball. The pack gate uses `npm pack`, the same tool family as `npm publish`, and derives the files it requires from the packed content instead of a hand-written list. Out of scope: bundling core into the package, running the real host in CI (OP-28), and rewriting the install docs (OP-29).
- **Repository evidence:** `tools/toolu-opencode/package.json` declares `@toolu/core ^7.4.0`, but core v7.4.0 exports only `bridge/config/decision/events/policy/runner`. The adapter imports `dispatch`, `gates`, `startup`, `registry`, `state`, `launcher` and `gates/*`. The published `@toolu/opencode@7.9.0` has 350 files, 62 of them `__tests__` files that import the private `@toolu/conformance`. `tooling/src/pack-inventory.ts` uses `bun pm pack`, while `npm-publish.yml` runs `npm publish`. The live scenarios pack a hand-staged copy with `--ignore-scripts`. `entry.full-startup` proves all 16 plugins only through the checkout shim with `TOOLU_REPO_ROOT` set.
- **Risk:** The live npm route installs `@toolu/core` from the registry, so the adapter must not import an unreleased core export (memory 6dbe117a). A gate that derives references from text could flag placeholders, so `<…>` and `…` forms are skipped explicitly.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Core dependency | `^<release>`, with a release-please `extra-files` JSON entry at `$.dependencies['@toolu/core']` | release-please's `GenericJson` replaces only the semver match, so the `^` survives. Publish order is core, then opencode. Jev compared exact against a hand-bumped caret and chose exact (0.98). The synced caret, found afterwards, keeps both properties: the floor always equals the release, and the existing caret convention and test stay | A future core minor could change behavior within the caret. Accepted, as it was before |
| Pack tool | `npm pack --json` (dry run for the gate, real pack for the live tarball) | Jev `npm` 0.99. npm reports file modes. Both tools produce the same 354-file list today | npm must be on PATH in CI. It is: the job already runs `npm install -g` |
| Tests in tarball | Exclude with `!src/**/__tests__` | Jev 0.91. npm and bun both honor the negation (290 files) | None |
| SDK peers | Optional peers pinned to `contract/pin.json` | Only `import type` uses them. Bun does not auto-install optional peers | None at runtime |
| Live proof | A new `package.clean-install` scenario: all 16 plugins, npm route, repo env blanked | `fullStartup` already counts modules and helpers through the shim | Runtime: one more full host session |

## Rejected

- **Bundle `@toolu/core` into a dist entry.** It would remove the version skew, but it replaces the TypeScript source exports and adds a build step, which is a bigger change than this work package needs.
- **Exact core pin.** It is as safe as the synced caret, but it contradicts the established caret convention and its test for no added guarantee in this release model.
- **Keep the hand-listed `required` files.** Each new helper needed a manual entry, which is how earlier issues added them one by one.
