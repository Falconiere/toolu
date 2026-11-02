---
name: verify-multi-machine-relay
description: Verify relay recovery, task transfer and peer conversations across two physical Herdr machines.
metadata:
  toolu:
    origin: agent
    created: 2026-11-02T16:28:47Z
---
## When to Use

Verify the relay across two physical machines after changes to delegation,
recovery, worker bootstrap or committed-result transport. Use the complete
procedure in [the delegation guide](../../../docs/delegation.md#two-physical-machine-verification);
the [recorded receipt](../../../docs/delegation-evidence.json) illustrates its evidence.

## Procedure

1. Prepare isolated controller and relay HC sessions, scratch configuration,
   distinct worker roots and matching committed origins. Connect the relay to
   each worker through an existing HC machine SSH target and a prestarted
   isolated remote core with the intended `PATH` and `TOOLU_CONFIG_DIR`.
2. Follow the guide to submit pinned work to both physical hosts. Record their
   hostnames, task, attempt and owned-terminal identities and inventories before
   disconnecting the controller. Stop and recover the owned relay service;
   inspect the same identities and inventories after recovery.
3. Verify the bound worker reports, imported commits and content, and repeat
   each Git result import. Record actual outputs and any manual intervention.
4. For [peer conversations](../../../docs/peers.md), use an explicit private group
   and key shared by the two scratch configurations. Discover the enrolled
   sessions, exchange a question and reply, and explicitly acknowledge receipt.
   Inspect outbox, inbox and acknowledgment receipts separately. Reconnect a
   queued sender and restart the owned relay; retries must retain their original
   envelope and leave one inbox entry. Keep task configuration and task state
   snapshots around messaging-only operations.
5. Exercise actual coding hosts in isolated profiles. Probe their normal login
   before isolating it, then link the existing credential into the private
   profile. Record explicit CLI messaging, context notices and idle wake support
   separately; a manual inbox check does not establish automatic notification.

## Pitfalls

- Use a short lowercase HC session name and a short `XDG_RUNTIME_DIR`; keep
  the relay config directory distinct from the staged Toolu binary path.
- HC exec cannot proxy a streaming HC connection. Connect through the configured
  machine target to the already running isolated remote core.
- Respect trust prompts for real code. The recorded run accepted trust only for
  the synthetic one-file fixtures; do not generalize that approval.
- HC accepts at most 8 MiB on piped writes. A compressed debug executable can
  exceed this too; use the existing chunked transfer or bounded chunks when
  staging a checkout binary.
- An HC core rejects a machine pointing back to itself. Give a controller and
  relay on the same physical host separate session names.
- A staged hook can use `TOOLU_BIN` while a coding host's login shell resets
  `PATH` to an older installed binary. Use the verified absolute checkout binary
  in the live messaging commands.
- Muse account login in a private XDG profile needs the current
  `muse/auth.json` under that profile's config root. `MUSE_AUTH_PATH` alone did
  not expose the account login. Use the host's own model catalog: Muse model IDs
  differ from Codex IDs, even when the work class is the same.
- Keep verification clones and temporary test files outside Git's internal
  directory. Stream checkout changes into an existing VM through standard input
  when the current worktree is not mounted; do not restart a shared running VM.
- Clean up only owned relay terminals and isolated cores; preserve the user's
  existing cores, credentials, worktrees and unrelated tasks. Stop the isolated
  controller before worker cores so its connections cannot restart them.

## Verification

Require distinct actual hosts, one owned attempt and terminal per task across
recovery, a valid bound success report, and matching committed content after
repeat import. Record failures and corrections instead of treating an idle
terminal or a submitted request as success.
