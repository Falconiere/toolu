# Language-neutral parity fixtures

**Outcome:** Every parity case named by #408 is a committed JSON record under `fixtures/`; TypeScript suites read those records and keep the same assertions and case counts. Rust tests can read the same records without evaluating TypeScript.

**Repository evidence:** `tooling/fixtures/{shell,config,portable-core,gate-coverage,codex-hook-schemas}` already holds JSON contracts. Lifecycle cases are mostly data records, while pre-tool and post-tool suites include functions that build real sandbox paths, git state, and registry scripts. Quality plugin cases have their own TS tables and JSON golden outputs. #409 already routes conformance tests through a shared entry seam.

**Decision:** Move the five existing contract trees to `fixtures/`, keep the three tooling-only trees (`conventions`, `guardrails`, `pr-babysit-herdr-smoke`) with their TS tools until #439, and document that boundary. Express dynamic setup as bounded JSON operations and templates interpreted by a TypeScript harness. Commit both case input and expected output JSON; no generator runs in tests. Record unbash parse results for every shell input as a differential baseline.

**Alternatives:** Captured stdin and outputs alone would lose sandbox and state setup semantics. A runtime generator from TS would leave TypeScript as the source of truth. Moving tooling-only overlays now adds path churn without a Rust parity consumer. Jev favored declarative JSON plus an interpreter (0.83 relative probability) and keeping tooling-only trees until #439 (1.00); the repository requirements and tests remain the authority.

**Risks:** Translating functions into descriptors can subtly alter path expansion or setup order. Before and after case counts, existing golden comparisons, conformance runs, and a fixture audit must catch drift. Broad path changes may affect docs and CI path groups, so those gates must run after the move.

**Handoff:** Write the design spec and its real-input acceptance checks, then a ledger plan by fixture family.
