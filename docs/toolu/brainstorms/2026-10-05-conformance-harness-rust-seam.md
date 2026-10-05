# Conformance harness Rust command seam

Brainstorm, 2026-10-05. Issue #409 in epic #402.

## Capsule

- **Outcome:** a conformance test names a plugin and entry once, then runs the committed Bun bundle by default or the selected Rust CLI command under `TOOLU_IMPL`. A missing selected binary fails at setup with its expected path.
- **Material defaults:** one resolver owns selector parsing, binary lookup and command arguments. Harness runners and the CLI conformance suites call it. The legacy epic script suites are explicitly assigned to #434/#435 for Rust black-box replacement.
- **Non-goal:** writing the Rust hook implementation or changing OpenCode's production bootstrap in this issue.
- **Repository evidence:** `@toolu/conformance/harness/spawn` already owns subprocess behavior; `pretool.ts` and `posttool.ts` own hook runs. `protected-dispatch.ts` spawns `pre-tools.js` directly. The Rust workspace currently has skeleton crates, so a real executable stub is the available integration probe. Of the `test:conformance` matrix, protected-files and spaces-cwd dispatch the hook; bootstrap-readiness tests OpenCode's missing-bundle diagnosis and surface-drift checks generated TypeScript output.
- **Risk:** default runs pass through the generated launcher, whose diagnostics and environment are part of existing parity expectations. Rust selection must not change those runs. The all-entries selector cannot imply that an unported implementation already exists.
- **Handoff:** write the observable spec, then the ledger plan and tests.

## Material decisions

The command resolver is the interface boundary. Scattering `TOOLU_IMPL` checks across callers would produce different selector, path and error behavior. Jev favored the central boundary over per-caller branching (0.84 vs 0).

After reading the actual `epic-watch.test.ts` and `report.test.ts` cases, the epic script decision changed: epic #402 redesigns the watcher as a resident engine; the old tests exercise legacy CLI flags and imported TypeScript functions. The issue explicitly allows their Rust coverage to move to #434/#435. With that evidence Jev favored naming replacement suites over inventing `toolu epic report/watch` verbs (0.99 vs 0.01). `report.test.ts` status/history/usage scenarios belong to #435's worker-report action; `epic-watch.test.ts` event, lock, checkpoint and timing scenarios belong to #434's engine state machine and #435's actions. The existing TypeScript tests continue to run for the TypeScript implementation.

The selector is a test switch, not persistent state. An absent selector keeps the current bundle command. `rust` selects all entries; `rust:<plugin>/<entry>,...` selects only exact entries. An empty port manifest should make the CI Rust leg a no-op while giving later ports one place to add selectors.

For selected hooks, the core plugin uses `toolu hook <entry>` as the epic's explicit command form. Other plugins use `toolu <plugin> hook <entry>` because several share `session-start` and `register`. Jev favored the namespaced form over a global entry-only command (0.88 vs 0.01).

The suite matrix includes two checks that do not execute a hook. They keep their current checks in both modes. The two hook-dispatch suites use the resolver and therefore exercise the selected Rust command. `protected-dispatch` is a shared helper and `spawn-check` is a subprocess helper, not independent matrix suites.

## Rejected approaches

- Per-caller branching: would duplicate parsing and produce inconsistent missing-binary failures.
- Map legacy epic script argv directly to uncommitted `toolu epic report/watch` verbs: the watcher is being redesigned, and no such argv contract exists.
- Force the OpenCode bootstrap and generated-surface checks through the Rust hook: they test different behavior and the native OpenCode shim is scheduled later in the epic.
