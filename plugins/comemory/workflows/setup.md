# Set up comemory

Set up the persistent-memory and code-index backend for the current repository.
The plugin safely no-ops when the `comemory` binary is absent.

1. Resolve `scripts/setup.sh` relative to this workflow file and run it with the
   caller's explicit `TOOLU_HOST_OVERRIDE` (`codex` from the Codex skill,
   `claude` from the Claude command). Pass `--force` only when the user
   explicitly asked to overwrite existing git hooks.
2. Read the first output token:

   - `READY`: report data directory, repository scope, hook installation,
     initial indexing, completions, and the `setup_done` marker. If existing git
     hooks were preserved, explain that an explicitly confirmed `--force` run
     replaces them.
   - `MISSING`: propose the exact printed install command. Prefer
     `brew install Falconiere/tap/comemory` when Homebrew exists. Run it only
     after explicit confirmation, then rerun setup.
   - `OLD`: propose `brew upgrade Falconiere/tap/comemory` when Homebrew exists,
     otherwise the printed installer. Run only after explicit confirmation,
     then rerun setup.
   - `ERROR`: relay the message and stop; do not retry blindly.

The setup is idempotent and never overwrites git hooks without `--force`.
Comemory is not published to crates.io; never use `cargo install comemory`.
