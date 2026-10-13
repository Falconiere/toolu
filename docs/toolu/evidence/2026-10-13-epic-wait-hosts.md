# `toolu epic wait` per host (#452, AC-5)

Date: 2026-10-13. Binary: `target/debug/toolu` 7.11.0 from this branch.
Engine: an isolated resident engine (`toolu epic engine --no-http`) with
`TOOLU_RESOURCE_HOME`, `EPIC_STATE_HOME` and `HOME` under `/tmp/w452/engine`,
herdr off (`TOOLU_EPIC_HERDR=off`); epic `demo-1` registered with
`toolu epic start`. Judgments were raised by `toolu epic report needs-human`
or `failed` on a status file of that epic, only after the waiter had started
(Claude) or returned `waiting` three times (Codex, OpenCode). The live epic's
state was never touched. `/tmp/w452/wait-logged.sh <host> 10` runs
`toolu --json epic wait --max-seconds 10` against that engine and logs each
call's start and end; the shorter limit replaces 480 only to keep the probe
short.

claude: PASS — Claude Code 2.1.296, this worker session. The skill's step:
`toolu --json epic wait` (no limit, the 2700 s default) as a background Bash
call. Started 14:45:38; `needs-human` reported at 14:45:59; the background
call returned at 14:45:59 and the harness woke the session with its
completion notice:

```text
{"age_ms":0,"epic":"demo-1","key":"demo-1-7","kind":"needs-human","note":"which base branch?","seq":1,"state":"open"}
exit 0
```

codex: PASS — codex-cli 0.162.1 (`codex exec`, model gpt-6-luna), prompted
with the skill's Codex/OpenCode loop rule:

```text
start 14:49:10
end 14:49:20 {"state":"waiting"}
start 14:49:22
end 14:49:32 {"state":"waiting"}
start 14:49:34
end 14:49:44 {"state":"waiting"}
start 14:49:47
end 14:49:47 {"age_ms":2267,"epic":"demo-1","key":"demo-1-8","kind":"failed","note":"codex probe event","seq":3,"state":"open"}
```

Codex's reply: `FOUND failed demo-1-8 after 4 calls`. The event was raised at
14:49:44, after the third `waiting`.

opencode: PASS — the repository's pinned `opencode-ai` 1.18.34 (`opencode run
-m openai/gpt-4.1`) in an isolated profile (`XDG_*` under a temp root, no
plugins), with the same prompt:

```text
start 14:54:25
end 14:54:35 {"state":"waiting"}
start 14:54:37
end 14:54:47 {"state":"waiting"}
start 14:54:48
end 14:54:59 {"state":"waiting"}
start 14:55:00
end 14:55:02 {"age_ms":0,"epic":"demo-1","key":"demo-1-9","kind":"needs-human","note":"opencode probe event","seq":4,"state":"open"}
```

OpenCode's reply: `FOUND needs-human demo-1-9 after 4 calls.` The event was
raised at 14:55:02, after the third `waiting`.

Not proven here: two earlier OpenCode attempts did not reach the loop. The
installed OpenCode 2.0.21 with the user's toolu plugin refused the bash call
(`toolu bootstrap: ... required registry/session artifacts are missing`), and
2.0.21 under an isolated profile timed out starting its background service.
toolu targets OpenCode 1.x, so the pinned 1.18.34 run is the evidence.
