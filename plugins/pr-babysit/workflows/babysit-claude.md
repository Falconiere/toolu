# Claude Code controller

### Claude Code

1. Use the shared target's `SLOT`. State: `STATE_FILE=/tmp/pr-babysit-${SLOT}.json`. See **Isolation invariants**.
2. Run the first check now with `toolu babysit tick` (Step 1) and act on it (Steps 2–6). The helper creates and initializes the state file on this first tick.
3. Then wait for work: run `toolu babysit wait --repo "$OWNER/$REPO" --pr "$NUMBER" --state-file "$STATE_FILE"` as a **background** Bash call. It checks every 180 s on a fixed schedule, without a model turn, and ends after at most 1800 s. Its output is either a tick result that needs you (success, escalation, pending work (`attention.needed`), or an active fixer) → Steps 2–6, then wait again; or the waiting document (`"state": "waiting"`) → start the next `wait` at once. Never sleep or poll yourself.
4. Tell user:
   > "Babysitting PR #N on branch `<branch>`, checking every 3 min. Auto-stops when CI is green and all comments are addressed. Say `/pr-babysit:babysit stop` to cancel."

First arg **`stop`**: resolve `SLOT` from current branch's PR → stop this slot's background `wait`, if one runs → `toolu babysit stop --state-file "$STATE_FILE"` (exits a live fixer, removes the clean herdr worktree, marks the state `cancelled`) → remove `/tmp/pr-babysit-${SLOT}.json` and its `.snapshot.json` and `.result.json` → confirm. Other slots untouched. Exit.
