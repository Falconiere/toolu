# pr-babysit in Rust — Brainstorm

**Date:** 2026-10-11   **Issue:** Falconiere/toolu#433 (epic #402)   **Path:** Full

## Capsule

- **Outcome:** `toolu babysit` replaces the Bun pr-babysit bundles. A tick is a
  deterministic library call checked every fixed 180 s; an LLM is involved only
  when the tick finds work. Inside an epic the engine owns the loop and prompts
  the worker once per distinct piece of work; outside an epic `toolu babysit
  wait` blocks until the PR needs action or is done.
- **Material defaults / non-goals:** same state file and result contract minus
  `backoff`; same strict clearance, slot isolation, injection handling and
  "never re-fetch" rule; GitHub through `toolu-github`, Jev through
  `toolu-jev-client`. No engine registration for PRs outside an epic (#447
  non-goal 3). No new crate dependencies.
- **Repository evidence:** precedent ports #504 (jev) and #510 (statusline)
  deleted `hooks/src` and `hooks/dist`, moved goldens to `fixtures/`, and
  rewrote the OpenCode generator and acceptance scenarios. The engine already
  calls `BabysitTick` through `crates/cli/src/links.rs` and maps
  `KeepGoing/Success/Escalate`; it prompts workers only with `STATUS?`.
- **Risk:** size (≈4k TS lines plus ≈2.8k test lines) against the 300-line file
  and 50-line function limits; the OpenCode acceptance scenarios and herdr
  smoke depend on helper command lines; live runs need GitHub and herdr.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| GitHub transport | `toolu-github` GraphQL + REST with ETag-free one-shot config for verbs, `Config::scheduled` for the engine | #460 built the client for pr-babysit; issue scope says so | Status-check rollup shape differs from `gh pr view`; normalise to the same `{__typename,name,status,conclusion,detailsUrl}` / `{context,state,targetUrl}` rows |
| Cadence | fixed 180 s, anchored slots in the state file (`schedule.nextCheckAt`) | issue: "every fixed 180 seconds, no backoff" | a crashed `wait` resumes on the next whole slot, never early |
| `wait` outside epics | in-process ticks, bounded by `--max-seconds` (per-host default) | #447 non-goal 3; Jev choice A (0.86) | none beyond host timeouts, which the defaults respect |
| Prompt de-duplication | the tick stores a fingerprint of the actionable work in its own state file and reports `act` only for a new one | Jev choice X (0.88); state file survives engine restarts; no plugin-to-plugin link | a worker that ignores a prompt is not prompted again until the work changes; the engine's stall nudge still applies |
| Engine prompt | new `TickDecision::Act` with prompt text; engine journals `prompt`/`babysit` and sends it through herdr `agent.prompt` (or the `prompt` script) | `server.rs` already sends `STATUS?` this way | `Escalate` keeps its attention path for errors only |
| Exit codes | `toolu_protocol::exit` codes; JSON error document unchanged | only 0/1/2/64/69/75 exist | callers that matched exit 3/4/5 change: documented in helper.md |
| Hooks | native `check-toolu` and `check-binary` SessionStart entries via `cargo xtask print-hook` | jev and toolu-review ports | none |
| Live acceptance | this PR's own babysit runs `toolu babysit wait` from the branch build against the real PR | no sandbox repository is authorised for this worker | recorded in the spec as the sandbox substitute |

## Alternatives rejected

- **`wait` registers with a running engine** — contradicts the approved #447
  non-goal and adds a socket protocol for a convenience; rejected.
- **Engine-side prompt memory** — needs world/snapshot schema changes and does
  not survive a reset of the engine's issue record; the tick already owns the
  per-PR state; rejected.
- **Keep shelling out to `gh`** — the issue names the shared client and the
  client already carries the retry knobs (`PB_GH_*`); rejected.
- **Add a SHA-1 crate** — one 8-hex-character finding key does not justify a new
  dependency; a small in-crate SHA-1 with the FIPS test vectors instead.
