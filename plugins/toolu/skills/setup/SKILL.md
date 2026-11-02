---
name: setup
description: Use when setting up toolu agent profiles, registered worker machines, or the delegation relay.
---

# Set up toolu agents

For machine and relay setup, register existing Herdr machines with `toolu setup
machines register`, then select the always-on relay with `toolu setup relay
--machine <name>`. Install the requested agent on each worker and use
machine-local repository credentials. Toolu uses a matching worker binary or
stages the relay binary privately on compatible platforms. Inspect `toolu
machines status` and each command's `--help` for its options. If relay
selection is interrupted, repeat setup with the same destination; a retired
controller must refresh the successor named by the error before registry edits.

`toolu setup agents` manages `quick-task`, `deep-explore`, `research-agent`, `implementer`, `test-author`, and `architect`.

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
