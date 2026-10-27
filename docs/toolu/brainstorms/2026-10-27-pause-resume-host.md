# Pause and resume an agent host — Brainstorm

**Date:** 2026-10-27   **Mode:** Delivery (compact→full: public interface + persistence)   **Issue:** Falconiere/toolu#653, part of epic #649

## Capsule

- **Outcome:** An operator pauses/resumes a host (`claude`, `codex`, `cursor`, `muse`) from `toolu epic hosts pause|resume <host>` and from `POST /api/v1/actions/host-pause|host-resume`. Paused hosts get no new agents; running workers keep running; resume also clears the host's usage-limit cooldown everywhere. `/api/v1/hosts` shows `paused`, `paused_by`, `paused_at`; the journal records `host-paused`/`host-resumed` with `by`.
- **Material defaults / non-goals:** No dashboard UI (#656), no fleet-peer control, no killing/relaunching running workers, no reroute of a paused host's waiting relaunch.
- **Repository evidence:** #650 (merged PR #671) built the token-gated action route (`http_action.rs` forwards the op verbatim with `by: web`; `dispatch.rs` matches distinct op strings). `limit.rs` writes cooldowns to two stores: the shared resource store (`write_cooldown`) and each epic's `state_dir/hosts.json` (`write_host`); `Capacity::load` merges both and `pick_host` skips cooling hosts. The issue's Review additions (2026-10-27) settle the storage shape (`paused_hosts: {"<host>": {"by": "user|web", "at": <epoch ms>}}` in the shared store, next to `cooldowns`), the relaunch rule, and the manual-launch refusal. Verified in the issue: `is_resource_state` accepts the new key as-is.
- **Risk:** Two cooldown stores make resume easy to half-clear (AC2 fails if `hosts.json` keeps the entry). Routing must tell paused apart from cooling in status while skipping both. CLI `hosts` is a new verb group name; `--host` is already a global flag elsewhere, so the host is a positional argument.
- **Handoff:** spec.

## Decisions (Jev-assisted, `toolu jev ask`, 2026-10-27)

| Decision | Chosen | Jev | Rejected |
| --- | --- | --- | --- |
| Relaunch of a running worker whose host is paused | Wait, reason `host paused`, no reroute; keeps its session for resume | `wait` 0.97 | Reroute to another host (loses session continuity) |
| Control-socket op shape | Two ops `host-pause` / `host-resume`, each `{host, by}` | `two_ops` 1.00 | One `host` op with an `action` field (special parsing in dispatch and `http_action`) |
| Enforcement layer | Skip in `Capacity`/`pick_host`/`keep` + manual-launch refusal (`--force` overrides); engine relaunch waits via routing | `routing_plus_manual` 0.92 | Hard guard inside `launch_issue` (turns a waiting relaunch into a failure) |

Defaults taken without a question (settled by the issue or its review additions): storage in the shared resource store; CLI sends `by=user`, web sends `by=web`; resume clears both cooldown stores and AC2 asserts `Capacity::load` reports neither cooling nor paused for every registered epic; manual launch refusal text `host <name> is paused; resume it or pass --force`; unknown host lists the valid hosts (CLI failure exit, HTTP 400).
