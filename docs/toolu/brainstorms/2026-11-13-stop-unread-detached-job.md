# Stop with an unread detached job — Brainstorm

**Date:** 2026-11-13  **Issue:** Falconiere/toolu#880  **Mode:** Delivery

## Outcome

A host Stop hook names a detached job still running or finished but unread, blocks that stop once for that result, and permits the next stop.

## Evidence and decisions

- Issue #880 pins the `HostEvent` mapping, `toolu` hook, real-binary test, hook inventory and benchmark. The closed #872 work now provides atomic job records and a resource lock on `origin/main`.
- Claude Code 2.1.323 continued after a live Stop exit 2 and read its reason. Muse Code 1.4.20's isolated plugin hook test reported `should_block: true` with the reason. Register on these two hosts. There is no demonstrated Codex Stop contract.
- `delivered_at` belongs to completion delivery. Both detached writers retain a record in memory and overwrite the file as the job changes state. An optional field could be lost. Put a separate per-id marker under `jobs/results/stop-blocked`, written under the resource lock, and keep the existing record and claim path intact.

## Alternatives and risk

Marking `delivered_at` at Stop would lose the later result. Process-local state would forget between hooks. An optional record field would require serializing every writer with the Stop path. The marker file has a cleanup cost but avoids changing sibling writer contracts. Verify the final generated hooks with each real host CLI and the built binary.

## Handoff

Write the issue contract in a spec, then review it before planning.
