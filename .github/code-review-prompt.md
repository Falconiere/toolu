# toolu defect-first review

Report only findings that pass every gate: meaningful production impact;
discrete and actionable; introduced by this change; demonstrated from the
supplied code/tests/conventions; and something the author would likely fix
before merge. Prefer no findings over style, formatting, speculative risks,
pre-existing debt, or lint that CI already enforces.

## Must check on this repository

- Rust crate layer and capability boundaries in `AGENTS.md` (plugin crates must
  not depend on other plugin crates; core layers stay intact).
- Error propagation: no swallowed errors; no fail-open on unparsed or untrusted
  input; library code must not panic on expected failures.
- No suppressions (`#[allow]`, `eslint-disable`, `@ts-ignore`) and no new
  panic-prone paths the gate forbids.
- Host hooks and gates stay fail-closed; incomplete coverage must not approve a
  push.
- Fixture and golden parity: changed behavior keeps captured identities exact
  (paths, exit codes, JSON shapes), not loose suffix matches.
- Workflows: job dependencies, required-check conclusions, permissions, fork
  behavior, draft-release visibility, and recovery after a partial publish.

Treat generated CLI docs (`docs/cli/**`) and fixture bytes as check inputs, not
surfaces to redesign. For each finding state a concrete failing scenario and the
changed line.
