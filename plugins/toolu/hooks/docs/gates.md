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

`ask` needs a host that can prompt. Codex cannot, so `ask` degrades there — to
`advise` for a judgement gate, and to **`block`** for a security guardrail
(see the warning below). The three guardrails ship on `ask`; every other gate
reaches it only via `gates.<name>.mode`.

## Presets

`gates.preset` sets every gate at once. The default is **balanced**.

| Gate | `strict` | **`balanced`** (default) | `relaxed` |
|------|----------|--------------------------|-----------|
| `pushReview` | block | **advise** | advise |
| `qualityGate` | block | **block** | advise |
| `commitGate` | block | **advise** | off |
| `planLedger` | block | **advise** | off |
| `docsSync` | block | **advise** | off |
| `agentTier` | block | **advise** | off |
| `bashCommands` ⚠️ | block | **ask** | ask |
| `protectedFiles` ⚠️ | block | **ask** | ask |
| `mcpBlocker` ⚠️ | block | **ask** | ask |

⚠️ = security guardrail. See the warning below.

## ⚠️ Guardrails now ASK instead of denying — read this

> **This changed the security posture of toolu. Read it before upgrading.**
>
> `protectedFiles`, `mcpBlocker` and `bashCommands` used to be **hard denies**
> you could not get past in-session. They now **ask you**, and a yes lets the
> call through.
>
> **What that means in practice:** an agent that wants to write your `.env`,
> rewrite a toolu hook, call a blocked MCP server, or run `node -e` no longer
> hits a wall. It puts a prompt in front of **you**, and you are the control.
> If you approve without reading, nothing protects you. The old behavior is one
> config line away — see *Restoring the hard denies* below.
>
> **Why:** the hard deny was unanswerable. A user could tell the agent "yes,
> edit that `.env.example`" and the agent was still refused, with deny text
> that claimed an override existed when none did
> ([#176](https://github.com/Falconiere/toolu/issues/176)). A guardrail nobody
> can answer gets routed around — via `Bash`, which the old check never saw —
> and a bypassed guardrail protects less than an honest prompt.

The three guardrails are in the preset table above, and every mode works on
them (`block` / `ask` / `advise` / `off`). Two things stay special:

1. **They ask at every preset except `strict`.** `relaxed` relaxes judgement
   calls; it does not silence a guardrail. `relaxed` means "stop lecturing me",
   not "write my `.env` without telling me".
2. **`ask` degrades to `block`, never to `advise`,** on a host that cannot
   prompt (Codex). Judgement gates degrade to `advise` — say it, don't stop the
   work. A guardrail fails **closed**: the whole point is that a human decides,
   so where no human can be reached the answer is no. Degrading these to
   `advise` would silently disable them exactly where nobody is watching.

The prompt itself is deliberately loud — a banner, the specific path or rule,
**why that thing is guarded**, and a note that approval covers one call only.
A guardrail prompt that reads like a routine "allow this?" trains people to
wave it through.

### Restoring the hard denies

```json
{ "gates": { "protectedFiles": { "mode": "block" },
             "mcpBlocker":     { "mode": "block" },
             "bashCommands":   { "mode": "block" } } }
```

Or `{"gates": {"preset": "strict"}}`, which blocks everything.

### Bash coverage

`protected-files` also inspects `Bash`/`Shell` commands for redirects, `tee`,
`sed -i`/`perl -i`, `cp`/`mv`/`install`, `dd of=`, `python -c open(..., "w")`
write targets, and command substitutions inside unquoted heredocs — so a
protected path cannot be written through Bash to dodge the prompt
(github.com/Falconiere/toolu/issues/176). Best-effort, not a full shell
grammar — see `bash_write_targets` in `hooks/lib/detect.sh`.

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
