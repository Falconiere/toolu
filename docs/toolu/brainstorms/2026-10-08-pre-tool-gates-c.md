# Pre-tool gates C in Rust — Brainstorm

**Date:** 2026-10-08   **Issue:** #422   **Mode:** Delivery

## Outcome

The native `toolu hook pre-tools`, `toolu hook agent-tier`, and `toolu hook mcp-tools` reproduce the existing push-review, plan-ledger, docs-sync, delegation, and MCP decisions and state effects. The 180 committed C cases and full A/B/C pre-tool suite run through the Rust binary.

## Evidence and choice

- #418 supplies an ordered `Gate` dispatcher; #419 and #420 fill six of nine pre-tool slots. #421 supplies ledger parsing, jq behavior, review-state checks, waivers, and docs glob helpers.
- TypeScript C gates have exact user messages, telemetry, mode handling, and read/write effects that differ from the report-only `toolu ledger verdict` gates.
- Use dedicated Rust `Gate` modules in the missing slots and reuse #421's state and parsing primitives. Add standalone hook adapters for agent-tier and MCP. Jev's comparison of shared primitives, direct verdict calls, and independent duplication selected this boundary with confidence 1 on the supplied evidence.

## Material boundaries

- Preserve the existing state formats, messages, ordering, host encoding, and fail-open or fail-closed behavior of each hook. Keep `hooks.json` on Bun until #425 switches hosts.
- The three workflow gates judge parsed pushes at the target repository, including `git -C` and detached refspec cases. The MCP entry avoids shell parsing. Agent-tier records delegation telemetry and only compares an explicit model with the selected ledger step.
- Prefer one shared push-target helper and existing Rust review/ledger/docs primitives. A direct call to verdict report gates loses gate modes and exact hook messages; copying their parsing into each gate would add divergence.

## Risks and checks

- The 180 C cases include state-file snapshots, warnings, host-specific asks, and malformed inputs; decision-only tests miss parity. Use the committed golden captures, standalone MCP tests through the native seam, and real Rust subprocess tests.
- The full A/B/C suite can reveal ordering or host-encoding regressions. Run it after the individual modules, then the complete Rust and Bun quality gates.
- Rust's 300-line files, 50-line functions, colocated tests, and coverage floor determine module boundaries. No gate-data exemption is part of this issue.

## Handoff

Draft the spec with observable parity, state and telemetry evidence, standalone hook contracts, and documentation impact; then review and plan within delivery-flow.
