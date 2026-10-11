# Set up the statusline

Wire the native statusline into the user's Claude Code `settings.json`. Claude
Code reads `statusLine` only from that file, so setup adds its command safely
and idempotently. It preserves a custom statusLine unless `--force` is given,
backs up an existing settings file before changing it, and upgrades older
commands that run the former published statusline path.

## Steps

1. Run setup:

   ```bash
   toolu statusline setup
   ```

   If the user explicitly asked to replace an existing custom statusLine, run
   `toolu statusline setup --force` instead.

2. Read the first word of the output (the STATUS token) and report:
   - **WIRED** / **CREATED** — success (WIRED also covers an older stable-path
     command). Tell the user to **restart the session** (or `/clear`) for
     the status bar to appear; `statusLine` loads at session start.
   - **ALREADY** — already wired; nothing to do.
   - **REFUSED** — a different statusLine is already set. Show the current value
     the command printed, and tell the user to re-run `/statusline:setup --force`
     to replace it (or wire it by hand).
   - **ERROR** — relay the message; do not retry blindly.

Do not hand-edit `settings.json` — setup round-trips the JSON and keeps a
`settings.json.bak` backup.
