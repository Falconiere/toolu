# Gates

Every toolu gate answers one question and then has to decide how forcefully to
say no. The question is the gate's own business; the answer's **delivery** is a
mode you configure.

## Modes

| Mode | What the user sees | What happens |
|------|--------------------|--------------|
| `block` | a denial with the gate's reason | the tool call does not run |
| `ask` | a prompt with the gate's reason | the user decides |
| `advise` | the reason, as context for the agent | nothing is stopped |
| `off` | nothing | the gate is silent |

`ask` needs a host that can prompt. Codex cannot, so `ask` degrades to `advise`
there — the gate still speaks, it just cannot hold the door. No preset
prompts: `ask` is opt-in via `gates.<name>.mode`.

## Presets

`gates.preset` sets every gate at once. The default is **balanced**.

| Gate | `strict` | **`balanced`** (default) | `relaxed` |
|------|----------|--------------------------|-----------|
| `pushReview` | block | **advise** | advise |
| `qualityGate` | block | **block** | advise |
| `commitGate` | block | **advise** | off |
| `bashCommands` | block | **advise** | advise |
| `planLedger` | block | **advise** | off |
| `docsSync` | block | **advise** | off |
| `agentTier` | block | **advise** | off |

`protected-files` and `mcp-blocker` are not in this table on purpose: they deny
at every preset, in every tool. A preset relaxes judgement calls, not
guardrails, and there is no session-level way to turn a `protected-files`
deny into an `ask` a user can approve — the deny reason says so rather than
implying otherwise. `protected-files` also inspects `Bash`/`Shell` commands
for redirects, `tee`, `sed -i`/`perl -i`, `cp`/`mv`/`install`, `dd of=`, and
`python -c open(..., "w"/"a")` write targets, so a protected path can't be
edited through Bash once the structured `Edit`/`Write` path is closed to it
(github.com/Falconiere/toolu/issues/176). That detection is best-effort, not
a full shell grammar — see `bash_write_targets` in `hooks/lib/detect.sh`.

## Per-gate overrides

```json
{ "gates": { "preset": "strict", "pushReview": { "mode": "ask" } } }
```

Precedence, first hit wins:

1. `gates.<name>.mode`
2. the legacy top-level key (`docsSync.mode`, `agentTier.mode` only)
3. the `gates.preset` table
4. the built-in `balanced` preset

An unrecognized value warns on stderr and falls through to the next layer, so a
typo mis-delivers nothing.

## The push waiver

`pushReview` in `ask` mode (opt-in) remembers a "yes":

1. The gate asks, and records a **pending** waiver naming the diff SHA it asked
   about (`.claude/tmp/push-review/<branch>.pending-waiver.json`).
2. `post-tools/modules/push-waiver.sh` sees the push actually ran **and
   succeeded**, and promotes the pending marker to
   `<branch>.waiver.json`. A rejected push promotes nothing — the pending
   marker survives for the retry.
3. The next push of that same diff passes silently.
4. A new commit changes the diff SHA, so the waiver stops applying and the gate
   asks once more.

A refused prompt leaves the pending marker orphaned; the state sweeper reclaims
it. Declining is not recorded as an answer — the gate will ask again.

## What the quality gate stops

`qualityGate` in `block` mode denies `git commit` and `git push`, and nothing
else. Reading, searching, editing, and running commands stay open while the
gate is red: a failing gate is a reason not to ship, not a reason to be unable
to work. `MY_CLAUDE_QUALITY=off` still disables it outright.

## Interaction with host permissions

`permissions.deny` in Claude Code settings **overrides** a hook's `ask` — a
command denied there is denied, and toolu never gets to prompt about it.
`permissions.allow` does not have that power: an allowlisted command still
reaches these gates. This is why toolu's shipped
`settings/permissions.fragment.json` no longer denies `git push --force` or
`node -e` — those are gate decisions now, and a deny rule would silently
outrank them.

## Tests

```bash
bats plugins/toolu/hooks/lib/__tests__/gate-mode.bats
bats plugins/toolu/hooks/lib/__tests__/push-waiver.bats
bats plugins/toolu/hooks/post-tools/modules/__tests__/push-waiver.bats
bats plugins/toolu/hooks/lib/__tests__/state-sweeper.bats
bats plugins/toolu/hooks/lib/__tests__/permissions.bats
```
