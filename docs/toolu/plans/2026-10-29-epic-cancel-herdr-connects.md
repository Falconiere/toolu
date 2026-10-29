# Cancel timed-out herdr connects — Plan

**Date:** 2026-10-29   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-29-epic-cancel-herdr-connects-design.md   **Topic:** Bound Unix socket connect lifetime for live HTTP requests (#747)

## Evidence and approach

`http_live::connect` currently spawns a blocking `UnixStream::connect` thread and drops its receiver after one second. `http_live_test.rs` already covers normal HerdrFixture sampling, missing socket, a silent server, and the eight-slot cap. `status_schema.rs` generates `docs/epic-status-api.md`. The workspace already uses `nix`; its safe socket API can give the connect a real deadline without detached threads or unsafe code. A real Linux backlog probe returned `EAGAIN`, then poll reported writable plus hangup while the socket remained unconnected, so the connector must retry `connect` until success or deadline.

## Workstream summary

Reproduce backlog retention with a real listener, replace the connector, document the cleanup, then run the full gate and final local reviews.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"Real backlog regression and bounded nonblocking herdr connect","check":"cargo test -p toolu-epic-orchestrator http_live","ac_refs":["AC-1","AC-2","AC-3","AC-4"],"paths":["Cargo.lock","crates/epic-orchestrator/Cargo.toml","crates/epic-orchestrator/src/http_live.rs","crates/epic-orchestrator/src/tests/http_live_test.rs","crates/epic-orchestrator/src/tests/http_live_backlog_test.rs"],"input":"Real UnixListener with backlog filled by connected UnixStream peers; repeated timed-out connects; accept after deadline and find no late request bytes; existing real HerdrFixture, missing socket and Silent server; held limiter permits. Measure threads and descriptors in release before and after.","model":"inherit"},
  {"id":"s2","title":"Release-build backlog and resource measurement on supported platforms","check":"cargo test --release -p toolu-epic-orchestrator backlog_connects_release_resources_at_deadline -- --nocapture","ac_refs":["AC-1","AC-2"],"depends_on":["s1"],"paths":["crates/epic-orchestrator/src/http_live.rs","crates/epic-orchestrator/src/tests/http_live_backlog_test.rs","docs/toolu/evidence/2026-10-29-epic-cancel-herdr-connects.md"],"input":"Real full Unix listen backlog on macOS and Linux; record release-build baseline and fixed samples, elapsed connection time, and before/after process thread and descriptor counts in the evidence file. macOS can refuse immediately; late acceptance is asserted only where connect stays pending.","model":"inherit"},
  {"id":"s3","title":"Generated status API documents immediate timed-out connect cleanup","check":"cargo test -p toolu-epic-orchestrator status_api_doc_matches_schema_definitions","ac_refs":["AC-3"],"depends_on":["s1"],"paths":["crates/epic-orchestrator/src/status_schema.rs","crates/epic-orchestrator/src/tests/status_schema_test.rs","docs/epic-status-api.md"],"input":"Generated API reference and the documented live herdr socket paragraph; check that regenerated text matches the committed reference.","model":"inherit"},
  {"id":"s4","title":"Full repository gate on final committed branch diff","check":"cargo xtask gate","ac_refs":["AC-5"],"depends_on":["s2","s3"],"paths":["crates/epic-orchestrator","docs/epic-status-api.md","docs/toolu/evidence/2026-10-29-epic-cancel-herdr-connects.md","docs/toolu/specs/2026-10-29-epic-cancel-herdr-connects-design.md","docs/toolu/plans/2026-10-29-epic-cancel-herdr-connects.md"],"input":"Complete branch diff with real Unix socket tests, generated docs, release measurements, and all repository quality checks.","model":"inherit"}
]
```

## Critical files

Modify `Cargo.lock`, `crates/epic-orchestrator/Cargo.toml`, `crates/epic-orchestrator/src/http_live.rs`, `crates/epic-orchestrator/src/status_schema.rs`, and generated `docs/epic-status-api.md`. Reuse the existing sibling `tests/http_live_test.rs` and add `tests/http_live_backlog_test.rs`, this plan, the reviewed spec, and `docs/toolu/evidence/2026-10-29-epic-cancel-herdr-connects.md`. Keep production files under the Rust quality limits.

## Verification

Run each ledger check through `toolu epic job --` when it is an expensive test or gate. The regression must first fail on the original connector, then pass with real backlog saturation, late acceptance, and resource counts on macOS and Linux. Regenerate the API reference and verify it. After a scoped commit, run `toolu ledger run <plan> --verify` and the full gate; push the branch. Run two independent medium `toolu code-review run --base origin/main` reviews on the pushed head and require both nonempty JSON reports, zero unreviewed files, and approvals. Create or verify a PR against `main` with the required issue and epic links. The orchestrator owns admin merge and `toolu epic finish`; this worker then idles.

## Plan review (2026-10-29)

- s2: 🟡 should-fix: release measurement and platform coverage were only in prose. Fixed by adding a measured release-build step and a durable evidence file. The macOS probe returned immediate `ECONNREFUSED`; the test accepts that bounded result and asserts late close where a connection can remain pending.
- Architecture: 🟡 should-fix: polling after Linux `EAGAIN` cannot prove connection success. Fixed by using short, deadline-bounded nonblocking `connect` retries.
- The user-specified pushed-head review happens after the ledger and push; the worker records both full-coverage reports before yielding the merge to the orchestrator.

**Status:** Approved
