---
description: Set up the comemory persistent-memory + code-index backend.
agent: build
---

# Set up comemory

First-time setup for the `comemory` persistent-memory + code-index backend.
The plugin is a thin wrapper over the `comemory` binary and **no-ops entirely
if that binary is absent** — this command detects it, guides the install when
it is missing or too old (it never runs a package manager itself), and once the
binary is present wires the current repo: the data directory, git hooks that
auto-refresh the code index on commit/merge/checkout, an initial index, and a
shell-completions hint.

## Steps

1. Run the setup script via the published stable wrapper. Pass `--force`
   through **only** if the user explicitly asked to overwrite pre-existing
   git hooks:

   ```bash
   bash "${OPENCODE_CONFIG_DIR:-$HOME/.config/opencode}/comemory/setup.sh" $ARGUMENTS
   ```

2. Read the first word of the output (the STATUS token) and report:
   - **READY** — the binary is present and current; the lines below the token
     show what was wired (data dir, repo scope, `install-hooks`, `index-code`,
     completions). Relay the `install-hooks` / `index-code` results. If
     `install-hooks` was skipped because hooks already exist, tell the user they
     can re-run `/toolu-comemory-setup --force` to overwrite them.
   - **MISSING** — the binary is not installed. Show the printed install command
     (`brew install Falconiere/tap/comemory`, or the curl installer) and tell the
     user to run it, then re-run `/toolu-comemory-setup`. Do **not** install it
     for them.
   - **OLD** — the binary is below the version floor. Show the printed
     `brew upgrade Falconiere/tap/comemory` command and ask the user to run it,
     then re-run `/toolu-comemory-setup`.
   - **ERROR** — relay the message; do not retry blindly.

The script is idempotent — re-running once wired re-checks and re-indexes, and
never overwrites existing git hooks unless `--force` is passed. comemory is
**not** published to crates.io, so the canonical install is the Homebrew tap or
the curl installer — never `cargo install comemory`.
