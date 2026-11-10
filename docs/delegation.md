# Remote worker removal

Toolu now runs workers locally through Herdr or tmux. Relay scheduling, machine
registration, remote execution, relay-backed peer conversations and the
hc/Herdr App worker backend are no longer supported. The read-only fleet
dashboard, peer HTTP views and their credentials remain supported.

Before upgrading an installation with active remote work, use its existing
binary to stop new admissions, settle or cancel queued/running tasks, import
committed results and retire its owned relay service. Keep that binary and its
configuration/state until the work is verified. Stop the old resident epic
engine before starting the updated binary.

The new binary rejects explicit retired backend choices and saved remote/hc/
relay worker state. Errors name the affected path and require a pre-removal
Toolu binary for recovery. A retired registration prevents the shared epic
engine snapshot from loading; settle that registration with the old binary.
Independent local manual lifecycle operations retain their existing behavior.

No workers are silently retargeted, uncertain leases released or reports marked
complete. State, receipts, messages, worktrees, transfer refs and credentials
are not deleted. Resolve the recorded work before explicitly changing local
backend configuration to auto, herdr or tmux. Do not erase identity fields to
make an unsupported record look local.

The upgrade does not uninstall Herdr, Herdr App or hc, or change remote machine
software. Cleanup of old installations or retained evidence is a separate
operational action.
