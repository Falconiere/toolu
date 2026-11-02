---
name: verify-multi-machine-relay
description: Verify relay recovery and committed result transfer across two physical Herdr machines.
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

## Pitfalls

- Use a short lowercase HC session name and a short `XDG_RUNTIME_DIR`; keep
  the relay config directory distinct from the staged Toolu binary path.
- HC exec cannot proxy a streaming HC connection. Connect through the configured
  machine target to the already running isolated remote core.
- Respect trust prompts for real code. The recorded run accepted trust only for
  the synthetic one-file fixtures; do not generalize that approval.
- Clean up only owned relay terminals and isolated cores; preserve the user's
  existing cores, credentials, worktrees and unrelated tasks.

## Verification

Require distinct actual hosts, one owned attempt and terminal per task across
recovery, a valid bound success report, and matching committed content after
repeat import. Record failures and corrections instead of treating an idle
terminal or a submitted request as success.
