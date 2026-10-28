//! Read-only Codex marketplace pin status from config.toml (#626).

use toolu_runtime::env::Env;
use toolu_runtime::host::codex;

/// Notice when Codex's toolu marketplace is not pinned to `v<version>`.
pub(super) fn notice(env: &Env, version: &str) -> Option<String> {
  codex::marketplace_pin_notice_from_env(env, version)
}

#[cfg(test)]
#[path = "tests/codex_pin_test.rs"]
mod tests;
