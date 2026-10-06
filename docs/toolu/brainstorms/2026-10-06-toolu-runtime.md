# toolu-runtime: host roots, config, settings, process, startup publish, registry types — brainstorm (#414)

**Date:** 2026-10-06   **Mode:** Delivery, Full path (a core crate's public interface; the fail-closed config envelope)

## Capsule

- **Outcome:** `crates/core/runtime` gains what every hook and verb needs to run on a host, without a shell parser or TLS: an explicit environment snapshot, host detection, the host roots and the Codex plugin snapshot, the `toolu.config.json` loader with its fail-closed envelope and per-key resolvers (gate modes, thresholds, models, docs-sync globs), the permissions write, the plugin settings files, bounded subprocesses with process-group kill, the startup helpers (stable-path publish, bounded context, Codex dependency warnings, `TOOLU_STARTUP_REPORT` records) and the registry types (module manifest and `Rule` trait).
- **Material defaults / non-goals:** no hook is ported and the `toolu` binary does not change; the consumers are #415 to #424. The strict whole-document schema (`TooluConfigSchema`) stays in TypeScript until its one consumer, the OpenCode selection reader, is ported (#462). The OpenCode status record stays with #462. The in-process registry runner is #418.
- **Repository evidence:** `packages/toolu-core/src/{host,config,process,startup,registry,cli}`, their `__tests__`, `fixtures/host/root.json` (18 cases), `fixtures/config/*.json` (16 envelopes, no expected outputs), the #413 precedent `fixtures/host/encode.json` with a TypeScript and a Rust test over one golden, and `tooling/conventions/guardrails/rust/rules.json` (`std::process::Command` only in `toolu-runtime`'s `process` module; `std::env` only in `toolu-runtime`).
- **Risk:** byte parity of messages that embed `JSON.stringify` output (the `unsupported version` text) and key order (the `unknown top-level keys` list). Both are covered by an ordered top-level read and the shared golden. Root runs on this host ignore `chmod`, so no test may rely on an unwritable directory (comemory `be52369e`).
- **Handoff:** spec.

## Axes and decisions

| Axis | Decision | Evidence | Jev |
|---|---|---|---|
| Parity oracle for the 16 config fixtures | Capture the TypeScript results once into a committed golden, `fixtures/config/expected.json`: for each fixture placed as the project file, the `invalid` text, the warnings and the resolved values. A Bun test and a Rust test both reproduce every case. | `fixtures/host/encode.json` (#413) uses the same pattern; no Rust test spawns Bun | `golden` 0.93 |
| Namespaced `epic` section | Accept `epic` as a known top-level key whose contents are never checked, in both the TypeScript loader and the Rust loader, so both implementations accept the same files during coexistence | Issue AC; epic "Strictness" bullet; TS fails closed on `epic` today | `both` 0.50 (agent concurs: a Rust-only key would make the two loaders disagree on one file) |
| Unknown-key message | Own envelope pass over the top-level keys in document order, not serde's `deny_unknown_fields` text | The hook-time TS loader checks only the envelope (`config-load.ts`); serde's message differs | — |
| Strict whole-document schema | Deferred to #462 | Only consumer is `tools/toolu-opencode/src/inventory/selection.ts` | `defer` 0.96 |
| Process-group kill | `nix` (already a workspace dependency) with its safe `signal` feature: `killpg` | `unsafe_code = "forbid"`; TS `signalProcessGroup` | `nix` 0.86 |
| Stable-path publish | Symlinks only, as TypeScript does: an absent path or a symlink is replaced; a regular file or directory is the user's and is kept. The issue's "launcher script" clause has no format anywhere yet; #443 owns the `toolu` on PATH design and can extend the owned set | `startup/publish.ts`; no launcher-script format in the repository | `symlink_only` 0.83 |
| Registry manifest | Strict: unknown fields and any `version` other than 1 are rejected, since a format change bumps `hookProtocol` | Epic "Registry" and "Distribution" bullets | `strict` 0.87 |

## Rejected alternatives

- **A Rust test that runs the TypeScript loader through Bun:** couples `cargo test` to Bun and the TypeScript tree, which #440 deletes.
- **Hand-written Rust assertions per fixture:** nothing would prove the two implementations agree.
- **`epic` in Rust only:** a file with an `epic` section would fail closed under TypeScript hooks and load under Rust hooks.
- **Spawning `kill(1)`:** a subprocess per signal, and the result depends on the user's `PATH`.
- **Marker-based launcher scripts now:** a format nobody writes yet.
