//! Whether Codex's toolu marketplace is pinned to this binary's release tag
//! (#626). An unpinned one follows toolu's main: at every session start Codex
//! re-clones it when main moved and reinstalls the plugins, and a reinstalled
//! plugin's hooks need review again, so a Codex epic worker stops at that
//! review before its first turn.

use toolu_runtime::env::Env;
use toolu_runtime::host::codex;

/// The notice for `$CODEX_HOME/config.toml` (else `~/.codex/config.toml`);
/// `None` when it is pinned to `v<version>`, absent or unreadable, or when
/// its `toolu` marketplace is not toolu's own.
pub(super) fn notice(env: &Env, version: &str) -> Option<String> {
  codex::marketplace_pin_notice_from_env(env, version)
}

#[cfg(test)]
#[path = "tests/codex_pin_test.rs"]
mod tests;
