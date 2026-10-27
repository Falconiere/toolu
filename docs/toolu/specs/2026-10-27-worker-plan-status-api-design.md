# Worker plan in the status API — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Falconiere Barbosa (epic worker)   **Topic:** Falconiere/toolu#652 — expose each worker's plan ledger through the epic status API

## Problem
The dashboard (epic #649) needs each worker's todo list. Agents rarely use their own todo tools, but every delivery-flow worker writes a plan ledger (`toolu ledger`) with per-step status. Today the status API only shows the phase, so an operator cannot see how far a worker has got.

## Non-Goals
1. No freshness recomputation: the engine never runs `git diff`. A step's `status` is the one stored in the ledger; `summary.stale` is the stored count.
2. No ledger writes from the engine, and no healing of orphaned `running` steps.
3. No UI work (#655), no new routes, and no peer proxy changes beyond carrying the field through its existing redaction.
4. No exposure of `check`, `evidence_tail`, `diff_sha`, `scope_sha`, `input`, `retries` or any other unlisted step field.

## Architecture
A new module `crates/epic-orchestrator/src/status_plan.rs` owns the projection:

- **Branch:** `head_branch(worktree)` reads `<worktree>/.git`. A directory means `<worktree>/.git/HEAD`. A file whose text is `gitdir: <path>` means `<path>/HEAD`, with a relative path resolved against the worktree. `ref: refs/heads/<b>` gives `<b>`. Any other content (a detached HEAD) gives `HEAD`, the same answer `git rev-parse --abbrev-ref HEAD` gives. No subprocess runs. The review addition suggested a cache keyed by the HEAD file's mtime. That is not done: finding HEAD already needs the `.git` read, and a stat costs the same as reading a 41-byte file (Jev choice `direct`, p=0.95). Because nothing is cached, AC6 holds by construction.
- **Ledger path:** a new `toolu_engine::ledger::io::ledger_path_for(root, branch, roots)` returns `<state root>/plan-ledger/<branch_slug>.json`. The existing `ledger_path` calls it, so both use one rule. The engine passes `Roots::new(Env::process(), Some(host))`, where `host` comes from `Host::parse` on the worker's host. That gives Claude Code `.claude/tmp/plan-ledger/`, Codex `.codex/tmp/plan-ledger/`, Cursor `.cursor/tmp/plan-ledger/` and Muse `.toolu/tmp/muse/plan-ledger/`. `TOOLU_PROJECT_CONFIG_DIRNAME` still overrides, as it does for the worker.
- **Read:** `toolu_engine::ledger::io::read_ledger` and `Ordered` are converted to `serde_json::Value`, then allow-listed field by field. `version` must equal 1.
- **Issue document:** `status_api::issue_document` adds `plan`. The worktree is `issues/<key>.json` `worktree`, falling back to `Issue::worktree`. The host is the same value the document already reports (`routes/<key>.json` `host`, else `kind`). Redaction stays `secrets::redact_json` over the whole document, and the control socket's `status` verb uses the same `document` function, so `toolu epic status --json <key>` gets the same field.
- **Epic detail:** each issue in `status_epics::detail` adds `plan_summary`. It is read through a process-wide `Mutex<HashMap<PathBuf, (SystemTime, Value)>>` keyed by ledger path and refreshed only when the file's mtime differs. `detail` runs on HTTP threads and on the state thread, hence the mutex. `route` and `document` already redact the detail document.

## Interfaces / Schema
```rust
// toolu_engine::ledger::io
pub fn ledger_path_for(root: &Path, branch: &str, roots: &Roots) -> PathBuf;
// crate::status_plan
pub(crate) fn plan(worktree: Option<&str>, host: Option<&str>) -> Value;          // object, {"error":"unreadable"} or null
pub(crate) fn plan_summary(worktree: Option<&str>, host: Option<&str>) -> Value;  // cached by mtime
```
`plan`:
```json
{"plan_doc": "docs/…-plan.md", "updated_at": "2026-10-27T…Z", "next": "s3",
 "summary": {"total": 9, "green": 7, "red": 0, "pending": 2, "running": 0, "stale": 0},
 "steps": [{"id": "s1", "title": "…≤160 chars", "status": "green", "activity": null, "last_run": "2026-10-27T…Z"}]}
```
`plan_summary` is `summary` plus `next`, `{"error":"unreadable"}` or `null`. A string field that is absent or not a string becomes `null`. A count that is not a non-negative integer becomes 0. `title` and `activity` are cut to 160 characters.

## Failure modes and edge cases
- No worktree recorded, an unknown or empty host, a missing `.git`, or an unreadable HEAD: `null`.
- No ledger file: `null`. The worker has not reached its plan phase yet.
- The file exists but is empty, is not JSON, is not an object, or `version` is not 1: `{"error":"unreadable"}`. The rest of the document is unchanged, and the error never propagates.
- `steps` is absent or not an array: `steps` is `[]`. A step that is not an object is skipped.
- Concurrent rewrites: the worker's `write_ledger` stages the file and renames it, so a read sees either the old or the new file, never a partial one. The cache is keyed by mtime, so a rename with a new mtime refreshes it.
- A poisoned cache mutex: the ledger is read uncached for that call.
- Secrets: configured secrets in `title`, `activity` or `plan_doc` are redacted by the existing `redact_json` pass. For peers, the proxy redacts again.

## Acceptance criteria
- **AC-1:** Given a git worktree whose ledger was written by `toolu_engine` `ledger_run` and refreshed by `ledger_status` under Claude Code roots, `GET /api/v1/issues/{key}` returns a `plan` whose steps (id, title, status, activity, last_run), summary counts and `next` equal the `ledger_status` ledger.
- **AC-2:** AC-1 holds for Muse (`.toolu/tmp/muse/plan-ledger/`), Codex (`.codex/tmp/plan-ledger/`) and Cursor (`.cursor/tmp/plan-ledger/`) worktrees.
- **AC-3:** With no ledger, `plan` is `null`. A corrupt ledger, or one at `version` 2, gives `{"error":"unreadable"}`. In each case `key`, `phase` and `phase_history` are still present.
- **AC-4:** A ledger whose step `check` and `evidence_tail` contain a sentinel string never leaks the sentinel through the issue route, the epic route, the control-socket status or the peer proxy.
- **AC-5:** `plan_summary` on `/api/v1/epics/{id}` keeps its cached counts after the ledger is rewritten with its previous mtime. It shows the new counts once the mtime changes.
- **AC-6:** After the worktree checks out another branch with its own ledger, `plan` reports that branch's ledger.
- **AC-7:** Through `/api/v1/fleet/peers/{name}/issues/{key}`, `plan` arrives with a configured secret in a step title redacted.
- **AC-8:** `toolu epic status --json <key>` over the control socket returns the same `plan` as the HTTP route.
- **AC-9:** A real linked worktree (`git worktree add`, `.git` is a file) resolves its branch the same way `git rev-parse --abbrev-ref HEAD` does, including a detached HEAD.
- **AC-11:** A step title of 400 characters arrives as its first 160 characters, and the step's `diff_sha`, `scope_sha`, `input` and `retries` keys are absent from `plan.steps[]`. A configured secret in a title is redacted in `/api/v1/issues/{key}`.
- **AC-10:** `cargo xtask gate` passes.

## Acceptance evidence
- AC-1/2: a temporary repo with a linked worktree and a real two-step plan document (one passing `true` check, one failing `false` check). `ledger_run` runs with `Roots::new(env, Some(host))` for each host, then `ledger_status`. The engine HTTP listener is started on a `World` holding the issue (meta `kind` = host). A test in `src/tests/status_plan_test.rs` compares the projected `plan` with the ledger JSON. Boundary: the `false` step is red and `next` is non-null.
- AC-3: same setup with no ledger, garbage bytes, and `version: 2`. Unit and HTTP assertions on `plan` plus the intact `phase_history`.
- AC-4: a ledger whose step `check`, `evidence_tail` and `input` carry `PLAN-SECRET-CANARY`. The issue, epic, control-socket and proxy responses are each checked with `!contains`.
- AC-5: write a ledger and read `plan_summary`. Rewrite it with different counts, then reset its mtime with `File::set_modified` and read again: the cached counts remain. Set a later mtime and read: the new counts appear.
- AC-6: in the linked worktree, `git checkout -b other` writes a second ledger; `plan` follows it.
- AC-7: extend `http_fleet_proxy_test.rs`. A real local HTTP peer (`peer_fixture::serve`) serves an issue document with a `plan` whose step title holds a secret configured on the proxying engine. The proxied body has the redaction marker and no secret.
- AC-8: a control-socket `status` request with `epic: <key>` against a served engine (`serve_with_http`) equals the HTTP `plan`.
- AC-9: `head_branch` against a `.git` directory, a `git worktree add` worktree, and `git checkout --detach`, each compared with `git rev-parse --abbrev-ref HEAD`.
- AC-11: a hand-edited copy of a real ledger with a 400-character title. Assert `chars().count() == 160` and that only the five allow-listed keys are present. With `Secrets` holding the title's secret, the issue route shows the redaction marker.
- AC-10: `cargo xtask gate`.

The tests drive `toolu_engine::ledger::run::ledger_run` and `commands::ledger_status`, the functions `toolu ledger run` and `toolu ledger status --json` call. They write the same ledger file in a real git repository without locating a built binary from the orchestrator crate's unit tests. The issue's test plan names the CLI; this is the same code path, one layer down.

## Documentation impact
`docs/epic-status-api.md` is generated from `status_schema.rs`. Add `plan` to the issue fields and `plan_summary` to the epic issues, plus a paragraph on the plan projection, per-host ledger directories, the `unreadable` error and the cache. Regenerate with `TOOLU_UPDATE_DOCS=1`. `docs/cli/` needs no change, because the `toolu epic status --json` shape is documented by this API reference.

## Open Questions
None blocking. The epic's design wants a per-step "stale" mark (#655). The engine reports the stored status, plus `summary.stale`, and #655 can render freshness from them. Owner: #655. Non-blocking.

## Spec review (2026-10-27)
- Acceptance criteria: 🟡 should-fix: the 160-character title cut and the local redaction of a secret in a title had no AC. Fixed by AC-11.
- Acceptance evidence: 🟡 should-fix: the tests use the ledger library instead of the `toolu ledger` binary. Justified: it is the same code path.
- Architecture: 🔵 consider: the branch cache from the review addition was dropped. The justification is recorded, and AC-6/AC-9 cover the behavior.
- Jev coverage score: 1.1 of 2 at low confidence (0.27). The manual check against each issue bullet found no remaining gap.

Every section is filled, every AC has runnable real-data evidence, and no blockers remain. **Status:** Approved
