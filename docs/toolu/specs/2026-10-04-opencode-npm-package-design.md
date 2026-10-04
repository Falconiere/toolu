# OpenCode independently loadable npm package — Design

**Date:** 2026-10-04   **Status:** Approved   **Author:** Claude Code   **Topic:** OP-27 (#361): ship a complete, independently loadable `@toolu/opencode`

## Problem

`@toolu/opencode` should work from npm alone. Today nothing proves that, and some of it is false:

1. **Dependency floor is wrong.** The package declares `"@toolu/core": "^7.4.0"`. Core v7.4.0 exports only `bridge`, `config`, `decision`, `events`, `policy` and `runner`. The adapter imports `@toolu/core/dispatch`, `gates`, `gates/mcp-hook`, `gates/agent-tier`, `startup`, `registry`, `state`, `launcher`, `config` and `policy`. A cache or lockfile that resolves any core version below the release fails to load the plugin. Nothing raises the floor at release time.
2. **Orphan files are shipped.** The published 7.9.0 tarball has 350 files. 62 of them are `src/**/__tests__` files that import the private `@toolu/conformance` harness, which no installed copy can resolve.
3. **The gate checks a different artifact.** `tooling/src/pack-inventory.ts` lists files with `bun pm pack`, but the publish workflow runs `npm publish`. The live scenarios pack a hand-staged copy with `--ignore-scripts`, so the package's real `prepack` never runs under test.
4. **Resources are hand-listed.** The gate's `required` list names a few helpers one by one. A new `$TOOLU_PLUGIN_ROOT_<X>/…` reference, Markdown link or relative import in a shipped file is not checked against the tarball. Nothing checks that executable bundles keep mode 0755, that every `exports` target ships, or that every bare import is a declared dependency.
5. **No clean-install proof.** All 16 plugins have been proven only through the checkout shim with `TOOLU_REPO_ROOT` set (`entry.full-startup`). The npm-route scenarios enable only one or two plugins.

## Non-Goals

1. Bundling `@toolu/core` into a built entry, or changing the TypeScript-source `exports` model.
2. Running the real host in CI. OP-28 (#362) owns that. This issue adds the live scenario and records its local run.
3. Rewriting the install or migration docs. OP-29 (#363) owns that. Only the facts this change alters are updated.
4. Changing adapter behavior, plugin startup, generated surface content, or Claude Code or Codex packaging.
5. Publishing a release by hand, tagging, or bumping versions.

## Architecture

1. **Manifest (`tools/toolu-opencode/package.json`).**
   - `files` becomes `["src", "!src/**/__tests__", "generated", "plugins"]`. npm and bun both honor the negation (measured: 354 → 290 files).
   - `dependencies["@toolu/core"]` becomes `^7.9.0`, the current release.
   - `peerDependencies` gains `@opencode-ai/plugin` and `@opencode-ai/sdk`, both `1.18.34` (the `contract/pin.json` SDK pin). `peerDependenciesMeta` marks both `optional: true`. The adapter uses them only through `import type`, the host supplies its own copy, and Bun does not auto-install optional peers. The existing `devDependencies` stay for local typechecking.
   - `exports` and `main` are unchanged. The closure gate now proves each target ships.
2. **Release sync.**
   - `release-please-config.json` gains `{"type":"json","path":"tools/toolu-opencode/package.json","jsonpath":"$.dependencies['@toolu/core']"}`. release-please's `GenericJson` replaces only the semver match inside the value, so `^7.9.0` becomes `^7.10.0` in the Release PR.
   - `npm-publish.yml`'s "tag matches every package version" step also fails when `@toolu/opencode`'s core floor differs from the tag.
3. **Pack listing (`tooling/src/pack-inventory.ts`).** `packedFiles` runs `npm pack --dry-run --json` in the package directory. That is the command family `npm publish` uses, and it runs the real `prepack`. The function returns `{ path, mode }` entries and parses the JSON array that starts at the first line beginning with `[`, so prepack chatter cannot corrupt it. `bundle-plugins.ts` writes its summary line to stderr. The `@toolu/plugins` and `@toolu/core` expectations are otherwise unchanged.
4. **Closure gate (new `tooling/src/pack-closure.ts`, run by `pack-inventory.ts` for `@toolu/opencode`).** Given the package directory, after prepack, and the packed `{path, mode}` list, it reports:
   - **References:** each `$TOOLU_PLUGIN_ROOT/<p>` maps to `plugins/toolu/<p>`. Each `$TOOLU_PLUGIN_ROOT_<NAME>/<p>` maps to `plugins/<name>/<p>`, where `<name>` is `NAME` lower-cased with `_` turned into `-`. Each `$TOOLU_OPENCODE_ROOT/<p>` maps to `<p>`. Braced and unbraced forms both count. The reference comes from any packed `.md` or `.json` file under `generated/` or `plugins/` and must name a packed file, or, when it ends in `/`, a packed directory prefix. Paths holding `<`, `…` or `*` are placeholders and are skipped.
   - **Links:** each relative Markdown link `](path)` in a packed `generated/**/*.md` resolves, after removing any `#fragment`, to a packed file. URLs and pure anchors are skipped.
   - **Imports:** for every packed `.ts` and `.js` file, `Bun.Transpiler.scanImports` reads the import specifiers. Relative ones must resolve to a packed file, either exactly or by adding `.ts`, `.js` or `/index.ts`. Bare ones must be a `node:` or `bun:` builtin or a name in `dependencies`. A `@toolu/core/<sub>` import must be a key in the workspace core's `exports`. Type-only imports are erased by the transpiler and therefore not checked, which is why the SDK can stay an optional peer.
   - **Exports:** `main` and every `exports` target are packed.
   - **Modes:** every packed `plugins/<x>/<p>` whose source `plugins/<x>/<p>` is executable has mode 0755.
   - **Forbidden:** any path containing `__tests__/` or `fixtures/` (added to the existing forbidden prefixes and patterns).
   The hand-listed helper entries in `required` are removed, because the reference rule derives them. `package.json`, `README.md`, `LICENSE`, `src/plugin/toolu.ts` and the committed bundles stay required.
5. **Real publish-path tarball for the live scenarios (`scenarios-entry.ts`).** `packTarball(workDir, edit)` builds `<work>/repo/tools/toolu-opencode` from the package's `package.json`, `README.md`, `LICENSE`, `src`, `generated` and `scripts`, and symlinks `<work>/repo/plugins` to the checkout's `plugins/`. It then runs `edit(stage)`, followed by `npm pack --json --pack-destination <work>` in the stage. The real `prepack` stages the catalog, and the same `files` field applies. The signature and callers stay the same.
6. **Clean-install scenario (new `tooling/src/opencode-host/scenarios-package.ts`, `package.clean-install`).** One session loads the tarball through the npm route. The project selects all 16 plugins, `TOOLU_REPO_ROOT` and `TOOLU_ROOT` are set to empty (overriding the merged process env), and the project lives in a temp sandbox outside the checkout. Bash in that session:
   - writes `$TOOLU_OPENCODE_ROOT` and `$TOOLU_PLUGIN_ROOT` to `roots.txt`;
   - runs `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"` into `status.txt`;
   - runs `"$TOOLU_BUN" "$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/scripts/report.ts" <file> brainstorm`;
   - runs `"$TOOLU_BUN" "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" path`;
   - appends each exit code to `markers.txt`.
   The scenario then reads the results from the filesystem:
   - The installed root is not under the checkout. Its file list, with `node_modules/` excluded, equals the tarball's `tar -tzf` list.
   - Every published helper symlink in the data root resolves inside the installed root.
   - Every `exports` target of the installed `package.json` imports in a fresh `bun` subprocess whose cwd is the sandbox. Bare dependencies resolve from the host's install, never from the checkout.
   - `toolu: ready (16 plugins, 12 startup artifacts)` is logged once, the registry modules match `entry.full-startup`'s list, and `.env` writes are still refused.

Decisive trade-off: the closure gate derives what must ship from the packed content, not from a hand list, so a new reference cannot ship broken unnoticed. The cost is a heuristic text scan. Placeholders are skipped explicitly, and a fixture test pins that behavior.

## Interfaces / Schema

```ts
// tooling/src/pack-inventory.ts
export type PackedFile = { path: string; mode: number };
export function packedFiles(directory: string): readonly PackedFile[]; // npm pack --dry-run --json
export function checkOne(expectation: Expectation, files: readonly string[]): readonly string[]; // unchanged

// tooling/src/pack-closure.ts
export type ClosureInput = {
  packageDir: string;            // the package root, after prepack
  files: readonly PackedFile[];  // the packed list
  sourcePlugins: string;         // <repo>/plugins, for the executable-bit source of truth
  coreExports: readonly string[]; // keys of packages/toolu-core/package.json exports, e.g. "./dispatch"
};
export function closureProblems(input: ClosureInput): readonly string[]; // one human-readable line each

// tooling/src/opencode-host/scenarios-package.ts
export const PACKAGE_SCENARIOS: EntryScenario[]; // [{ id: "package.clean-install", … }]
```

Problem line formats (stable, asserted by tests):

- `@toolu/opencode: <file> references <ref>, which the tarball does not contain`
- `@toolu/opencode: <file> links <link>, which the tarball does not contain`
- `@toolu/opencode: <file> imports <spec>, which the tarball does not contain`
- `@toolu/opencode: <file> imports <spec>, which is not a declared dependency`
- `@toolu/opencode: <file> imports <spec>, which @toolu/core does not export`
- `@toolu/opencode: export <key> → <target> is not in the tarball`
- `@toolu/opencode: <file> lost its executable bit (mode <octal>)`

`package.json` (additions):

```json
"files": ["src", "!src/**/__tests__", "generated", "plugins"],
"dependencies": { "@toolu/core": "^7.9.0", "gray-matter": "4.0.3", "zod": "4.1.5" },
"peerDependencies": { "@opencode-ai/plugin": "1.18.34", "@opencode-ai/sdk": "1.18.34" },
"peerDependenciesMeta": { "@opencode-ai/plugin": { "optional": true }, "@opencode-ai/sdk": { "optional": true } }
```

## Failure modes and edge cases

- **Prepack fails** (for example, fewer than 16 manifests): `npm pack` exits non-zero. `packedFiles` throws with stderr, and the gate and live pack both fail. Nothing is published, because the publish workflow runs the same prepack.
- **Prepack prints to stdout:** the JSON is parsed from the first line that starts with `[`. The summary moves to stderr anyway.
- **Placeholder references** (`$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/<helper>.js`, `${TOOLU_PLUGIN_ROOT_<PLUGIN>}`, `…/generated/…`): skipped. A bare variable with no path (`$TOOLU_PLUGIN_ROOT_PR_BABYSIT`) is not a reference.
- **Directory references** ending in `/` (`${TOOLU_OPENCODE_ROOT}/generated/`): satisfied when any packed path starts with the reference.
- **Fragment links** (`file.md#section`): the fragment is removed before resolving. External links (`https:`), `mailto:` and pure anchors are skipped.
- **Type-only imports:** erased by `scanImports`, so they are not checked. `import type` from the SDK is therefore fine as an optional peer.
- **A core export that is in the workspace but not yet on npm:** the hermetic gate passes, because it checks the workspace exports. The live scenario then fails at load (`toolu: ready` absent), which is the existing guard (memory 6dbe117a). The spec records it, and the scenario's observed output names the missing ready line.
- **Concurrent packs:** the gate runs `npm pack --dry-run` in the real package directory, which rewrites the gitignored `plugins/`. The live tarball packs in its own temp stage, so the live smoke and the gate never share a stage. As before, `test:pack` and the unit test `npm-publish.test.ts` each run the inventory serially.
- **The host's install cache is reused across runs:** each run packs into a fresh `mkdtemp` and uses a `file:` spec with a unique path, so the host installs a fresh copy.
- **Host env leaks the checkout:** `run()` merges `process.env`. The scenario sets `TOOLU_REPO_ROOT=""` and `TOOLU_ROOT=""` explicitly, and the adapter treats blanks as unset (`nonEmpty`).
- **Missing `npm`:** `packedFiles` throws `npm pack … failed`. CI has npm (the job installs global CLIs with it).

## Acceptance criteria

- **AC-1:** Given the repository's `@toolu/opencode` package, `npm pack --dry-run --json` lists no `__tests__` or `fixtures` path. The packed list includes every `exports` target. Every packed `plugins/*/hooks/dist/*.js` whose source is executable has mode 0755.
- **AC-2:** Given the packed `@toolu/opencode` list, the closure gate reports zero problems. Given fixture packages that each break one rule (a missing referenced helper, a broken Markdown link, a relative import outside the tarball, an undeclared bare import, a `@toolu/core/<sub>` not in core's exports, a missing export target, and a lost exec bit), it reports exactly the matching problem line. Given a placeholder reference, it reports nothing.
- **AC-3:** Before the change, the closure gate run on the current manifest reports the shipped `__tests__` imports of `@toolu/conformance` as undeclared dependencies.
- **AC-4:** `tools/toolu-opencode/package.json` declares `@toolu/core` as `^<own version>` and `@opencode-ai/plugin` and `@opencode-ai/sdk` as optional peers equal to `contract/pin.json`'s SDK version. `release-please-config.json` has the `$.dependencies['@toolu/core']` extra-file entry. `npm-publish.yml` refuses a tag that differs from the core floor. `bun run check:opencode-host` fails when a peer pin differs from the contract pin.
- **AC-5:** On the pinned host (`opencode-ai@1.18.34`), `bun run smoke:opencode-entry package.clean-install` passes. The tarball comes from the real `npm pack` with `prepack`, and the session has empty `TOOLU_REPO_ROOT` and `TOOLU_ROOT`. The installed tree equals the tarball list, the roots sit outside the checkout, and all 16 plugins become ready. The registry modules on disk match `entry.full-startup`'s list. A `.env` write is refused through the tarball's `plugins/toolu/settings` runtime data. The listed helpers exit 0, every export imports, and every helper symlink resolves inside the installed root.
- **AC-6:** The existing npm-route scenarios (`entry.npm-root`, `surfaces.npm-clean`, `surfaces.lifecycle`, `cli.install`) still pass with the publish-path tarball. `bun run test` passes.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | `tools/toolu-opencode` after prepack | 290-ish files, no tests, modes kept | negation honored by npm | `bun run tooling/src/pack-inventory.ts` (in `test:pack`) |
| AC-2 | the real package, plus temp fixture packages built from real files on disk | 0 problems for the real package, one exact line per fixture | placeholder and directory refs, fragment links, type-only imports | `bun test tooling/src/__tests__/pack-closure.test.ts` |
| AC-3 | the current manifest before the `files` change | `imports @toolu/conformance/…, which is not a declared dependency` | — | red run of the AC-2 test, recorded in the ledger |
| AC-4 | the committed manifests and workflow | the assertions hold | a peer differing from the pin fails the contract | `bun test tooling/src/__tests__/npm-publish.test.ts tooling/src/__tests__/opencode-host-contract.test.ts` and `bun run check:opencode-host` |
| AC-5 | pinned `opencode-ai` CLI, the scripted provider, a fresh tarball | scenario `pass` with observed fields | repo env blanked; host cache reused across runs | `bun run smoke:opencode-entry package.clean-install` |
| AC-6 | same host | each named scenario `pass` | real prepack path | `bun run smoke:opencode-entry entry.npm-root surfaces.npm-clean surfaces.lifecycle cli.install`; `bun run test` |

## Documentation impact

- `docs/opencode.md`: the package statement, which says it ships committed bundles and runtime data, notes the clean-install proof. The live scenario list gains `package.clean-install`. The generated mirror `tools/toolu-opencode/generated/resources/repo/docs/opencode.md` is regenerated.
- `AGENTS.md`: the `tooling/src/pack-inventory.ts` row and the npm paragraph state that the inventory runs `npm pack` with the closure gate.
- `tools/toolu-opencode/README.md`: none. It does not list dependencies.
- `pack-inventory.ts` and `npm-publish.yml` header comments.

## Open Questions

None blocking. The live scenario runs locally, and adding it to CI is OP-28's job (owner: #362).

## Spec review

Jev alignment: AC coverage scored 2.72 out of 3 (P(full coverage) = 0.73). Scope creep came back at 0.40, which is uncertain. The optional SDK peers and the release sync answer the issue's scope bullets (a) and (c), so they are not extra scope.

- Acceptance criteria: 🟡 should-fix: AC-5 did not name the runtime-data (`settings`) or registry-module assertions that the architecture lists. Fixed by stating both in AC-5.
- No blockers. Every AC has a real input and a runnable check. Approved.
