# Pre-tool gates B in Rust — brainstorm

**Date:** 2026-10-07  
**Outcome:** `toolu hook pre-tools` evaluates bash-commands, commit-gate, and quality-gate in Rust with the observable TypeScript decisions and messages.

## Decision

Add three `Gate` implementations to `toolu-engine` in the existing built-in order. Reuse `toolu-shell` for each simple command, `toolu-runtime` for settings and config, and `toolu-state` for gate-file reads and git facts. Keep the Bun hook active until #425.

## Evidence

- Issue #420 requires all 110 cases in `fixtures/gates/pre-tool-modules-b.json`, relevant #283 cases, and a TypeScript/Rust gate-state lifecycle.
- #418 supplies the engine dispatch and `Gate` contract; #419 and #423 already populate its ordered built-ins.
- Existing TypeScript gates and golden fixtures define allow overrides, dynamic commit messages, host-specific ask behavior, and quality-file rendering.
- Jev preferred native gate modules over the Bun bridge (0.80 relative probability). For the quality file, it preferred `toolu-state::read_gate_file` with legacy-field extraction from `Unrecognized` values (0.99).

## Alternatives and risk

Keeping these gates in the Bun bridge would preserve a runtime dependency on every pre-tool call and would not meet the issue's Rust-port goal. Strict-v1-only reads would stop honoring legacy failing files still covered by committed fixtures. The main implementation risk is shell-analysis and message parity on unusual command forms; the committed fixtures and a live interleaving test will expose it.

## Boundaries

No hook launcher switch or gate-policy redesign. The existing state writer owns the gate-file format and lock; this issue only reads that state before a commit or push.

## Handoff

Write the spec with exact case coverage, failure behavior, documentation impact, and runnable checks.
