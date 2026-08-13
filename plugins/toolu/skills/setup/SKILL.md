---
name: setup
description: Use when installing, previewing, updating, backing up, or removing toolu's custom Codex agent profiles.
---

# Set up toolu agents

Use the bundled [installer](scripts/setup.sh). It manages `quick-task`,
`deep-explore`, `research-agent`, `implementer`, and `architect` under
`${CODEX_HOME:-$HOME/.codex}/agents`.

1. Run `bash <installer-path> preview` and show the exact plan.
2. For installs and managed upgrades, run `bash <installer-path> install`.
3. If preview reports an unmanaged conflict, inspect only the named file and
   ask for explicit confirmation before `install --force`. The script creates a
   timestamped backup before replacement.
4. For removal, show preview and ask for explicit confirmation before
   `remove --yes`. Use `--force` only after separately confirming any unmanaged
   conflict. Removal moves profiles into a timestamped backup.
5. Report the backup path and tell the user to restart Codex so agent profiles
   reload.

Never edit agent files by hand or infer confirmation from the original setup
request when a conflict or removal is involved.
