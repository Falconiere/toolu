## Outcome

Run toolu in OpenCode with verified gate enforcement while preserving existing Claude Code/Codex behavior. Build reusable, typesafe TS logic with thin host adapters and retain tested Bash implementations where they remain the right source of truth.

This epic is the specification and delivery tracker. Planning updates do not authorize implementation; no code is changed by this review.

## Required architecture and tooling

- **Bun throughout TS:** dependency management, workspace orchestration, scripts, build, generation and `bun test`; one root `bun.lock` and frozen installs. No second package manager or TS test runner.
- **Strict TS + Zod:** validate external events, config/manifests, subprocess JSON and persisted state at boundaries; derive types with `z.infer`. Use discriminated outcomes and explicit failure handling; no unchecked casts/`any` to bypass validation.
- **Shared logic:** `packages/toolu-core/` owns host-independent schemas, normalization and policy. `tools/toolu-opencode/` consumes it through explicit exports and owns OpenCode integration/content generation. Host SDKs stay out of the core; typed interfaces isolate runtime I/O. A Bun conformance CLI is a second real consumer. Existing hosts keep their native Bash path and gain no mandatory Bun dependency.
- **Existing conventions:** #213 reuses/adapts [toolu-conventions](https://github.com/Falconiere/toolu-conventions) at a pinned revision: oxlint/oxfmt, strict typecheck, structure and lint-integrity checks, dead-code and duplication gates. Adapt library/plugin boundaries; do not import an unrelated application stack or duplicate rule ownership.
- **Unified release:** root, shared core, wrapper and all Claude/Codex plugin manifests remain on one numeric `X.Y.Z`, released as `vX.Y.Z` by release-please.

## Enforcement contract

Bash fallback is an implementation choice, not an enforcement substitute. A shell-out result still needs a verified host interception point.

- Retain tested Bash behavior unless a native/TS implementation has a concrete portability benefit and conformance coverage. These are Bash scripts, not portable POSIX-sh scripts; Bash remains an explicit prerequisite for fallback features.
- An enabled pre-action deny must prevent the action. Never translate deny to prompt advice or trust the model to comply. Commit/push/plan/docs gates must evaluate actual tool commands and cwd before execution.
- Preserve ordering, enabled-plugin selection, ask/deny precedence, advisory merging, multi-file normalization, post-tool feedback and gate state. Post-tool blocks cannot undo completed edits.
- Verify actual OpenCode CLI/SDK versions and capabilities in #205. Distinguish plugin load from session lifecycle. Document existing host differences, including ask behavior, rather than asserting identical APIs.
- Inventory every behavior in #209. Required unsupported enforcement is a release blocker; only inherently host-specific behavior may be explicitly N/A. Unsupported versions and failed bootstrap must not appear to provide working enforcement.

## Sub-issues and delivery order

All rows below must also be attached as native GitHub sub-issues. Initial contracts/inventory feed implementation; evidence is completed at the end.

| Issue | Deliverable                                                                      | Prerequisites                                                            |
| ----- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| #205  | Shared contracts, Bun/Zod architecture and pinned host capability evidence       | Start here; coordinate with #209                                         |
| #209  | Exhaustive coverage inventory, implementation ownership and drift checks         | Start alongside #205; finalize mappings against it                       |
| #213  | Adopt/adapt conventions TS guardrails with pinned provenance                     | #205 package/boundary decisions                                          |
| #208  | Bun workspace skeleton and mandatory CI/strict TS quality gates                  | #205, initial #213 configuration                                         |
| #210  | Shared core, Zod contracts and real Bash execution bridge                        | #205, initial #209, #213, #208                                           |
| #211  | OpenCode host/config, selected plugins, helpers and registry lifecycle           | #205, initial #209, #213, #208; shared interface with #210               |
| #206  | Complete generated skill/agent/command/resource surface                          | #205, #209, #213, #208; coordinate with #211                             |
| #204  | Thin OpenCode adapter with actual pre/post enforcement, including language gates | #205, #209, #213, #208, #210, #211; integrate #206                       |
| #212  | Cross-host conformance, clean-install/lifecycle and real-session evidence        | #204, #206, #208, #210, #211; validate #209/#213                         |
| #207  | Unified release integration and final install/update/uninstall documentation     | Prepare after #205; final readiness requires all implementation and #212 |

CI and guardrails land before feature TS. The core and bootstrap may proceed in parallel after agreeing interfaces; these issues are not all independently shippable. Documentation and test-case design can start early, but do not advertise complete OpenCode support before final acceptance.

## Definition of done

- [ ] #205 records compatible CLI/SDK/Bun versions, supported platforms, enforceable host mechanisms, performance budgets and explicit failure policies.
- [ ] Every discovered hook/concern/lifecycle/helper/content behavior has a #209 row, implementation owner and test/evidence link; no required gate is unowned, unimplemented or disguised as N/A.
- [ ] Shared TS logic is consumed by both the adapter and a Bun conformance CLI without SDK leakage or duplicated contracts; Zod validates external data and strict type/guardrail checks pass.
- [ ] Required core and enabled TS/Rust/Python gates work in OpenCode. Real denied edits/commands have no side effects; multiple calls per prompt, patches, MCP/subagents and lifecycle events are covered according to the matrix.
- [ ] Runtime installation, selected-plugin dependencies, config precedence, resource paths, concurrent worktrees/sessions, reload/upgrade/disable/uninstall and stale registry handling are tested.
- [ ] Generated artifacts include supporting scripts/references/workflows, preserve permissions/model intent and load from a separate consumer project; regeneration and inventory checks show no drift.
- [ ] Full `bun run test` retains the existing Bash suite, shellcheck, context-budget and benchmark gates and adds mandatory TS checks. Tests use real tools/filesystem/runtime, with no mocks per AGENTS.md.
- [ ] #212 records actual supported-host smoke evidence, deterministic conformance, supported platform coverage and measured overhead. Claude Code/Codex retain their existing behavior and prerequisites.
- [ ] One release-please bump keeps every manifest aligned, frozen Bun install/build succeeds, and #207 documents a proven install/update/uninstall flow and truthful limitations.

## Non-goals

Dropping existing hosts; bulk Bash-to-TS rewrites; routing all existing hosts through TS; removing Bash where fallback remains; npm publication for the initial port; importing Cloudflare/UI/database application scaffolds; changing unrelated language policies. This review changes GitHub planning only.

## Review findings addressed

The original plan had duplicate sub-issue sections and no native child links; no implementation owner for the shared core; unsafe prompt-only gate mappings; assumptions about universal `register.sh` and success status; missing standalone/lifecycle/language routes; Markdown-only resource conversion; late/optional CI and mocked Bash tests; and incomplete release/install validation. #210–#213 fill the missing ownership, while #204–#209 now carry precise scope, dependencies and measurable acceptance criteria.

## References

- [OpenCode V2 plugin API](https://opencode.ai/v2/docs/build/plugins) — confirm against a pinned executable/SDK before implementation.
- [OpenCode skills](https://opencode.ai/v2/docs/skills), [agents](https://opencode.ai/v2/docs/agents), [commands](https://opencode.ai/v2/docs/commands).
- [toolu-conventions audited baseline](https://github.com/Falconiere/toolu-conventions/tree/3562c63eb29a05bdb3cbcae5380196043c159759) — source-specific adoption links in #213.
- Repo review baseline: `1d5addae4eff942e5b51d71f6d5cd64234a536e6`; AGENTS.md remains authoritative for contributing/testing rules.
