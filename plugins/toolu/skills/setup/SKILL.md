---
name: setup
description: Use when installing, previewing, updating, backing up, or removing toolu's custom Codex agent profiles.
---

# Set up toolu agents

`toolu setup agents` manages `quick-task`, `deep-explore`, `research-agent`, `implementer`, and `architect`.

1. Run `toolu setup agents preview` and show the exact plan.
2. For installs and managed upgrades, run `toolu setup agents install`.
3. If preview reports an unmanaged conflict, inspect only the named file and
   ask for explicit confirmation before running `toolu setup agents install`
   with `--force`. The command creates a timestamped backup before replacement.
4. For removal, show preview and ask for explicit confirmation before
   `toolu setup agents remove` with `--yes`. Add `--force` only after separately
   confirming any unmanaged conflict. Removal moves profiles into a timestamped
   backup.
5. Report the backup path and tell the user to restart Codex so agent profiles
   reload.
6. Run `toolu doctor` and `toolu config validate`, and present their output.

Never edit agent files by hand or infer confirmation from the original setup
request when a conflict or removal is involved.
