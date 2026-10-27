# Dashboard worker logs — brainstorm

**Outcome:** An operator can follow a worker's managed job output and Claude Code background Bash output from a token-gated Logs tab. Existing command results keep their current shape.

**Boundary:** Local engine routes only. Codex and Muse expose managed jobs; arbitrary redirected command output is outside the feature.

**Evidence:** `run_managed_job` owns the lease token and issue key, while `toolu_runtime::process` already drains both pipes. The screen route supplies the token and redaction pattern. The drawer supplies visible-only polling. A real Claude task directory has regular `.output` files and a symlink to a subagent transcript, so discovery must exclude symlinks.

**Decision:** Add an optional chunk observer to the existing process drain, and store each job's log plus metadata under the resource root. The observer sees bytes before the returned `Output` budget drops them. Keep a monotonic logical offset while the physical file retains only 8 MiB. Resolve every requested ID from the listing for that issue. Inspect only regular files below the exact Claude worktree slug. Jev favored the observer (1.00) and per-job sidecars (0.90) over process duplication, shell tee, or a shared index.

**Claude task state:** Treat a task as `running` while its file was recently modified and the worker is active; otherwise `done`. The private `.output` format has no completion marker in the observed directory. Jev favored this bounded heuristic (0.72); document its approximation.

**Risk:** A rolling file's logical offset must remain coherent across an 8 MiB trim and concurrent HTTP read. Secret redaction may expand JSON, so cap the response after redaction. An unreadable file becomes an available-false document. A crash can leave a lease briefly stale; listing uses the resource lease state after reconciliation.

**Alternatives:** A shell tee changes process and stream behavior and conflicts with the native rule. A second runner duplicates group termination and timeout logic. A shared index adds cross-job contention. Parsing Claude's private session transcript adds another unstable format.

**Handoff:** Write the spec, then a ledger plan with real managed-job, local HTTP, Claude file, and browser behavior checks.
