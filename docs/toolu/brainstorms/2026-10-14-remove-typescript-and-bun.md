# Remove the TypeScript implementation and the Bun prerequisite — Brainstorm

**Date:** 2026-10-14   **Issue:** Falconiere/toolu#440 (epic #402)   **Path:** Full

## Capsule

- **Outcome:** No TypeScript product code, no Bun bundle and no Bun step remain.
  Every hook on Claude Code and Codex runs the native `toolu` binary or fails
  closed with the install commands. `cargo xtask gate` passes on a machine
  without Bun. The npm wrapper `@toolu/plugins` keeps publishing.
- **Material defaults / non-goals:** no Windows binary is added, so Windows
  hooks report the missing binary instead of running Bun. Gate decisions, state
  formats and registry order do not change. Historical `docs/toolu/` plans keep
  their TypeScript references. Merge and release order stay with the
  orchestrator.
- **Repository evidence:** `crates/core/protocol/src/launcher.rs` (Bun fallback
  and `TOOLU_RUNTIME=bun`), `crates/core/engine/src/registry/bridge.rs` (the
  `.js` bridge), and `crates/xtask/src/{ci_yaml,check_workflows,dist,bench,package_workspace}.rs`
  (gate steps that spawn Bun). `tooling/src/benchmarks/hook-resources.ts`,
  `tooling/src/benchmarks/statusline.ts` and `tooling/src/ci-changes.ts`
  still run in CI. More than 100 Rust test files mention Bun or TypeScript,
  and a dozen of them spawn `bun` for parity.
- **Risk:** the epic requires one release with the Bun fallback. v7.11.0 does not
  contain #425, so merging this PR before a #425 release would skip the dogfood
  release. Deleting the parity tests removes the TypeScript oracle; the shared
  JSON goldens in `fixtures/` remain the contract.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Bun fallback (#425) | Remove it in this PR. The PR body and the orchestrator report say it merges only after a release that contains #425 | Epic: one release with the fallback; issue: remove after dogfood; worker cannot merge | The orchestrator merges early. Mitigated by the stated precondition (Jev 0.59 vs keep 0.34) |
| Windows `commandWindows` | Report the missing native binary: enforcing events exit 2, context events print a `systemMessage` | No Windows target in `release-native.yml`; #411 | Windows users lose hooks they had with Bun (Jev 0.80) |
| npm wrapper | One committed dependency-free ESM `cli.js` in `tools/toolu-cli/npm`, tested by Rust integration tests that run it with `node` | 200 lines of TypeScript, built by `bun build` (Jev 0.77) | Node must be on the test PATH; CI runners have it |
| JS toolchain | Minimal private root `package.json` (release-please `node` anchor, `jscpd` devDependency) with `package-lock.json`; `npm ci` | release-type `node`; the gate's jscpd step (Jev 0.91) | Lockfile switch triggers every CI group once |
| Workflow YAML in xtask | A pure-Rust YAML parser in place of `bun -e Bun.YAML.parse` | `ci_yaml.rs` | New dependency must pass `cargo deny` |
| Third-party `.js` modules | Never run. One SessionStart advisory names each `.js` module and points to the executable protocol | Issue scenario | A third-party plugin silently loses its rule until republished |
| Stable-path shims | Stop publishing `jev.sh`, `statusline.sh` and `write-state.sh`. Remove the symlinks toolu published, and keep any user file | Issue scope; `crates/{jev,statusline,toolu-review}/src/session.rs` | An agent that remembered `jev.sh` now has to use `toolu jev` |

## Alternatives rejected

- **Keep the Bun fallback and its bundles.** That keeps `packages/toolu-core`, the
  bundles and the Bun launcher, which are most of what this issue deletes.
- **Drop `commandWindows`.** Codex on Windows would then run the POSIX command
  under cmd.exe and get a parse error instead of a clear message.
- **Keep the npm wrapper in TypeScript.** That contradicts the acceptance that
  no `.ts` remains, and it keeps `bun build` in the release path.
- **Switch release-please to another release type.** That would mean
  re-tagging and a manifest change for no user benefit.
