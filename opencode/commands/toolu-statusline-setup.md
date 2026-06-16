---
description: Set up the toolu statusline.
agent: build
---

# Set up the statusline

Wire the statusline into the user's opencode config so the TUI footer renders
toolu status. opencode does not have a statusline bar in the same way Claude
Code does — the closest equivalent is a footer toast surfaced by the toolu
adapter on a short interval. This command adds the periodic refresh into the
adapter's config and ensures the wrapper is published. It is idempotent and
never clobbers an existing custom status hook.

## Steps

1. Run the setup script via the published stable wrapper. Pass `--force`
   through **only** if the user explicitly asked to replace an existing
   custom status hook:

   ```bash
   bash "${OPENCODE_CONFIG_DIR:-$HOME/.config/opencode}/statusline/setup.sh" $ARGUMENTS
   ```

2. Read the first word of the output (the STATUS token) and report:
   - **WIRED** / **CREATED** — success. Tell the user to **restart the
     session** for the footer indicator to appear; opencode loads the adapter
     config at session start.
   - **ALREADY** — already wired; nothing to do.
   - **REFUSED** — a different status hook is already set. Show the current
     value the script printed, and tell the user to re-run
     `/toolu-statusline-setup --force` to replace it (or wire it by hand).
   - **ERROR** — relay the message; do not retry blindly.

Do not hand-edit the opencode config — the script round-trips the JSON and
keeps a `.bak` backup.
