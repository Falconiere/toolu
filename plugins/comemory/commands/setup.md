# Set up comemory

First-time setup for the `comemory` persistent-memory + code-index backend.
The plugin is a thin wrapper over the `comemory` binary and **no-ops entirely
if that binary is absent** — this command detects it, installs or upgrades it
when missing or too old (confirm-gated, since that touches the system), and once
the binary is present wires the current repo: the data directory, git hooks that
auto-refresh the code index on commit/merge/checkout, an initial index, and a
shell-completions hint.

## Steps

1. Run the setup script. Pass `--force` through **only** if the user explicitly
   asked to overwrite pre-existing git hooks:

   ```bash
   bash "${CLAUDE_PLUGIN_ROOT}/scripts/setup.sh" $ARGUMENTS
   ```

2. Read the first word of the output (the STATUS token) and act on it.
   Installing or upgrading the binary is **confirm-gated**: it modifies the
   system, so propose the exact command and run it only after the user gives an
   explicit yes.
   - **READY** — the binary is present and current; the lines below the token
     show what was wired (data dir, repo scope, `install-hooks`, `index-code`,
     completions, and the `setup_done` marker). Relay the `install-hooks` /
     `index-code` results and confirm memory is enabled (`setup_done` marker
     written). If `install-hooks` was skipped because hooks already exist, tell
     the user they can re-run `/comemory:setup --force` to overwrite them.
   - **MISSING** — the binary is not installed. Detect whether `brew` is on the
     PATH; if so propose `brew install Falconiere/tap/comemory`, otherwise
     propose the curl one-liner the script printed. Ask the user to confirm, and
     **only on an explicit yes** run that command, then re-run `setup.sh` and
     report the new STATUS.
   - **OLD** — the binary is below the version floor. Propose
     `brew upgrade Falconiere/tap/comemory` (or the curl installer if `brew` is
     unavailable), ask the user to confirm, and **only on an explicit yes** run
     it, then re-run `setup.sh` and report the new STATUS.
   - **ERROR** — relay the message; do not retry blindly.

The script is idempotent — re-running once wired re-checks and re-indexes, and
never overwrites existing git hooks unless `--force` is passed. comemory is
**not** published to crates.io, so the canonical install is the Homebrew tap or
the curl installer — never `cargo install comemory`.
