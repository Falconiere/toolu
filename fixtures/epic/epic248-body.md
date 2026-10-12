## Outcome and agreed decisions

Track full logical replication of engine-owned shared data between machines and the console. This is a delivery epic; each linked issue owns implementation and its regression tests. No implementation is requested in the issue-creation session.

**Latest user correction (2026-09-21): the daemon is required and always resident.** Installation and updates must start/restart it immediately; normal CLI startup repairs a missing service. CLI commands retain their existing execution cores. This supersedes the earlier login-only/optional daemon design: no `--no-daemon` option, and logout must leave the daemon running but unauthenticated.

Other agreed decisions:

- Bidirectional shared data: memory records, snippet-free code metadata/graph, extracted documents, and retained feedback/activity.
- The workspace engine orders changes by server acceptance, not client clocks. Idempotent retries retain their original acceptance. Tombstones require an explicit permitted restore; an ordinary stale edit cannot resurrect data.
- Code snippets, checkout paths, credentials, machine configuration, raw session transcripts, candidate observations/judgments, and job state stay local. Platform projects, billing, and authentication are not mirrored onto laptops.
- Local CLI execution remains independent of remote network availability. No physical SQLite copying or database-provider migration.
- Healthy authenticated propagation target: p95 <=2 seconds for an idle, connected pair under the test profile; five-second reconciliation discovers lost notifications. Backlogs must drain without requiring another nudge.

## Work breakdown and dependencies

- [ ] https://github.com/Falconiere/comemory/issues/249 — [Realtime replication] Establish the real-process test harness and permanent CI coverage gates
- [ ] https://github.com/Falconiere/comemory/issues/250 — [Realtime replication] Add a versioned journal, operation receipts and server-ordered protocol
- [ ] https://github.com/Falconiere/comemory/issues/251 — [Realtime replication] Capture every memory mutation and recover interrupted markdown writes
- [ ] https://github.com/Falconiere/comemory/issues/252 — [Realtime replication] Replicate code generations bidirectionally without overwriting local indexes
- [ ] https://github.com/Falconiere/comemory/issues/253 — [Realtime replication] Sync portable document revisions and searchable remote caches
- [ ] https://github.com/Falconiere/comemory/issues/254 — [Realtime replication] Sync feedback and activity with exactly-once effects and preserved provenance
- [ ] https://github.com/Falconiere/comemory/issues/255 — [Realtime replication] Drain durable push/pull backlogs safely across failures and policy changes
- [ ] https://github.com/Falconiere/comemory/issues/256 — [Realtime replication] Preserve replica state through bootstrap, rebuild, purge and recovery
- [ ] https://github.com/Falconiere/comemory/issues/257 — [Realtime replication] Make the daemon required, always resident and self-healing
- [ ] https://github.com/Falconiere/comemory/issues/258 — [Realtime replication] Start and verify the required daemon after every managed install or update
- [ ] https://github.com/Falconiere/homebrew-tap/issues/1 — [Realtime replication] Keep the required daemon running across Homebrew install, upgrade and formula regeneration
- [ ] https://github.com/CodaSignal/comemory.io/issues/183 — [Realtime replication] Gate replica APIs by workspace and current repository policy
- [ ] https://github.com/CodaSignal/comemory.io/issues/184 — [Realtime replication] Relay committed engine changes to workspace channels with reconnect recovery
- [ ] https://github.com/CodaSignal/comemory.io/issues/185 — [Realtime replication] Refresh all shared console views and expose actionable sync status

The hard dependency graph is stated in each child. https://github.com/Falconiere/comemory/issues/249 starts early; it is not a final-only testing task. Dependencies mean implementation prerequisites, not automatic release activation. The epic closes only after every child and the complete matrix below have evidence.

## Coverage matrix / epic acceptance

| AC  | Observable outcome                                                                                                                     | Owning issues                                                                                                                                                                                                                                                                                                                                                             |
| --- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E-1 | Installed/upgraded CLI has one healthy daemon on the expected binary, before login; logout keeps it alive                              | https://github.com/Falconiere/comemory/issues/257, https://github.com/Falconiere/comemory/issues/258, https://github.com/Falconiere/homebrew-tap/issues/1                                                                                                                                                                                                                 |
| E-2 | CLI/HTTP/MCP/console/job writes converge across devices without manual sync                                                            | https://github.com/Falconiere/comemory/issues/251, https://github.com/Falconiere/comemory/issues/252, https://github.com/Falconiere/comemory/issues/253, https://github.com/Falconiere/comemory/issues/254, https://github.com/Falconiere/comemory/issues/255, https://github.com/CodaSignal/comemory.io/issues/184, https://github.com/CodaSignal/comemory.io/issues/185 |
| E-3 | Offline backlog, >2,000 entries, filtered pages, lost acknowledgements and restarts lose no acknowledged data and double-count nothing | https://github.com/Falconiere/comemory/issues/250, https://github.com/Falconiere/comemory/issues/255, https://github.com/Falconiere/comemory/issues/256                                                                                                                                                                                                                   |
| E-4 | Server acceptance ordering and explicit deletion/restore semantics hold under races and retries                                        | https://github.com/Falconiere/comemory/issues/250, https://github.com/Falconiere/comemory/issues/251, https://github.com/Falconiere/comemory/issues/252, https://github.com/Falconiere/comemory/issues/253                                                                                                                                                                |
| E-5 | Local snippets, source registrations, pending writes and per-workspace ownership survive pulls                                         | https://github.com/Falconiere/comemory/issues/252, https://github.com/Falconiere/comemory/issues/253, https://github.com/Falconiere/comemory/issues/255, https://github.com/CodaSignal/comemory.io/issues/183                                                                                                                                                             |
| E-6 | Authorization/revocation, redaction, purge, migration/rebuild, old/new clients and failed upgrades have enforced regressions           | https://github.com/CodaSignal/comemory.io/issues/183, https://github.com/Falconiere/comemory/issues/256, https://github.com/Falconiere/comemory/issues/258, https://github.com/Falconiere/homebrew-tap/issues/1                                                                                                                                                           |
| E-7 | Every child ships meaningful edge-case tests and persistent CI guardrails                                                              | https://github.com/Falconiere/comemory/issues/249, all children                                                                                                                                                                                                                                                                                                           |

## Delivery guardrails

- Required checks: engine `bash scripts/check-all.sh`, `cargo nextest run --all-features`, `just e2e`; platform `bun run check`; the real cross-repository suite and actual macOS/Linux lifecycle checks.
- Pin both repositories' exact tested revisions/artifacts. Do not test an arbitrary binary found on PATH or silently substitute current main.
- Deploy compatible engine first, then platform forwarding/relay, then clients/console. Advertise full capability only when migrations and all supported entity handlers are ready. Keep old protocol compatibility; document rollback limits when a new schema was opened.
- Each implementation issue owns its changed public docs. Put the engine's durable design in committed `docs/designs/`: `docs/toolu/` is ignored in this repository. Toolu's plan/ledger may be working artifacts but cannot be the only durable contract.
- Existing platform issues remain separate: [learning operation permissions](https://github.com/CodaSignal/comemory.io/issues/153), [workspace/account deletion](https://github.com/CodaSignal/comemory.io/issues/80), and [personal-workspace consent](https://github.com/CodaSignal/comemory.io/issues/86). This epic must not silently enable those scopes or claim to close those issues.

## Review record

**Planning audit completed 2026-09-21.** Read back all 15 published issue bodies from GitHub and checked them against the reviewed drafts. Verified all 14 native sub-issue links and all 40 native blocker relationships; the prerequisite graph is acyclic. Every implementation issue has acceptance criteria, real-data edge scenarios, runnable verification, documentation ownership and the no-mock/CI evidence requirement.

| Issue                                                | Specific gap/edge coverage reviewed                                                                   |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| https://github.com/Falconiere/comemory/issues/249    | Real runtime prerequisites, zero-test/skip rejection, artifact pinning and bounded teardown.          |
| https://github.com/Falconiere/comemory/issues/250    | Immutable payloads, replay receipts, epoch changes, metadata hashes and read-only routes.             |
| https://github.com/Falconiere/comemory/issues/251    | Crash recovery, all mutation surfaces, pending edits, deletion/restore and vector compatibility.      |
| https://github.com/Falconiere/comemory/issues/252    | Local snippet preservation, divergent checkout heads and incomplete generation activation.            |
| https://github.com/Falconiere/comemory/issues/253    | Portable identity, partial-scan deletion safety, atomic searchable revisions and source ownership.    |
| https://github.com/Falconiere/comemory/issues/254    | Exactly-once counter effects, provenance, journal retention and activity-loop prevention.             |
| https://github.com/Falconiere/comemory/issues/255    | >2,000 entries, filtered pages, failed imports, lost acknowledgements and policy races.               |
| https://github.com/Falconiere/comemory/issues/256    | Legacy bootstrap, live rebuild, erasure-manifest-safe restore and schema rollback limits.             |
| https://github.com/Falconiere/comemory/issues/257    | Required residency, independent CLI execution, concurrent self-repair and logout auth barrier.        |
| https://github.com/Falconiere/comemory/issues/258    | Immediate verified startup, actual running version, interrupted updates and unmanaged-channel limits. |
| https://github.com/Falconiere/homebrew-tap/issues/1  | Real Homebrew lifecycle, stable binary path and generated-formula preservation.                       |
| https://github.com/CodaSignal/comemory.io/issues/183 | Tenant/repository isolation, revoked access, staged uploads and old/new protocol compatibility.       |
| https://github.com/CodaSignal/comemory.io/issues/184 | Post-commit job notifications, content-free machine frames and supervisor/relay teardown.             |
| https://github.com/CodaSignal/comemory.io/issues/185 | Fresh-cache reconnect, missed final nudge, workspace-switch races and honest fleet status.            |

The review corrected these gaps before completion: required install/update daemon lifecycle replaces optional login-only behavior; logout keeps the service alive but cannot silently reauthenticate from inherited credentials; expired event text is removed from journal/export copies without losing deduplication; rollback restores require current erasure barriers and a new stream epoch; local snippets/source registrations and pending local operations remain protected.

**Known installation boundary is explicit, not waived:** bare binary copies/`cargo install` bypass managed completion hooks; the supported source wrapper finalizes daemon startup and normal CLI startup repairs unmanaged placement. Homebrew immediate-start behavior must be proven with real package operations before that channel claims support. These are tracked acceptance gates in https://github.com/Falconiere/comemory/issues/258 and https://github.com/Falconiere/homebrew-tap/issues/1, not claims that installation has already been fixed.

**Audit scope:** issue specification and coverage only. No product files were changed and no implementation tests were run or claimed passing. The required suites must be implemented/run with the feature PRs; this planning audit cannot guarantee future implementation correctness.
