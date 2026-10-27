# Live agent status probe (#651)

**Date:** 2026-10-27. herdr 0.9.5. The isolated session `toolu-651-probe` ran headless (`herdr --session toolu-651-probe server`) and was deleted afterwards. The engine was `target/debug/toolu epic engine --http 127.0.0.1:7791` from this branch, run under `env -i` with a temp `TOOLU_RESOURCE_HOME` and `TOOLU_EPIC_HERDR_SESSION=toolu-651-probe`. One registered epic `probe` had issue `probe-claude` (`running`, kind `claude`) mapped to the herdr agent named `probe-claude`.

## AC-1: status change reaches the issue document and /agents/live

A sampler read `herdr agent get probe-claude`, `/api/v1/issues/probe-claude` and `/api/v1/agents/live` every 200 ms for 60 s (290 samples) while the agent ran one prompted turn:

```
{"change": "idle->working", "at": 1793110468.529, "lag_s": {"issue": 0.0, "live": 0.62}}
{"change": "working->idle", "at": 1793110475.999, "lag_s": {"issue": 0.21, "live": 1.44}}
{"samples": 290, "titles": ["Notes.txt file listing"]}
```

The issue document followed herdr within one sample (0.0 s and 0.21 s), with no `/agents/live` dependency. `/agents/live` followed within 1.44 s, including its 2 s cache. The journal holds exactly two `herdr-event` rows for `probe-claude` (14:14:28.422Z and 14:14:36.011Z), one per change. It holds no `herdr-cli` row after the ~290 live polls.

AC-1: pass

## AC-2: one sample per cache window

Two back-to-back requests returned the same `sampled_at` (`2026-10-27T14:15:18.909Z`). Nine concurrent requests against a fast herdr all answered `200` from the shared cache. The `503 herdr_busy` path needs herdr to stall; `http_live::tests::overlapping_requests_past_eight_get_herdr_busy_while_overview_answers` covers it with a socket that never answers.

## AC-3: herdr unavailable

- `TOOLU_EPIC_HERDR=off`: `200` in 0.0005 s with `{"available":false,"items":[],"reason":"off",…}`. `/api/v1/overview` returned `200`. AC-3 off: pass
- Probe session stopped (`herdr --session toolu-651-probe server stop`): `200` in 0.0006 s with `{"available":false,"items":[],"reason":"not running",…}`. The issue document showed `agent_status: unknown`, and `/api/v1/overview` returned `200`. AC-3 herdr stopped: pass

## AC-6: four-host probe

The probe started one agent per host with the engine's bypass flags and sampled `herdr agent list` every 0.3 s for 150 s through one prompted turn ("run ls, write notes.txt"). Each host wrote `notes.txt`. Consecutive distinct `status/title` pairs:

```
probe-claude (Claude Code 2.1.311): blocked (trust dialog) → idle/Claude Code → working/Claude Code → working/Notes.txt file listing → idle/Notes.txt file listing
probe-codex (codex-cli 0.164.0):    blocked (trust dialog) → done/ws-codex → working/ws-codex → working/List files and save notes | ws-codex → done/List files and save notes | ws-codex
probe-cursor (Cursor Agent 2026.10.22): idle/Cursor Agent → working/Cursor Agent → working/List Files And Write → done/List Files And Write
probe-muse (Muse Code 1.4.13):      idle/ws-muse → working/ws-muse → done/ws-muse
```

On the default session, the engine-launched Claude Code workers of epic #649 (started with `-n <key>`) showed their key as the title ("falconiere-toolu-650") while `working`, not a task summary.
