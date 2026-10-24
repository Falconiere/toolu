//! Whether Codex's toolu marketplace is pinned to this binary's release tag
//! (#626). An unpinned one follows toolu's main: at every session start Codex
//! re-clones it when main moved and reinstalls the plugins, and a reinstalled
//! plugin's hooks need review again, so a Codex epic worker stops at that
//! review before its first turn.

use std::path::PathBuf;

use toolu_runtime::env::Env;

/// Where toolu's own marketplace comes from, as Codex records it.
const ORIGIN: &str = "https://github.com/Falconiere/toolu.git";

/// The fix every notice names.
const FIX: &str = "toolu plugins update --host codex";

/// The notice for `$CODEX_HOME/config.toml` (else `~/.codex/config.toml`);
/// `None` when it is pinned to `v<version>`, absent or unreadable, or when
/// its `toolu` marketplace is not toolu's own.
pub(super) fn notice(env: &Env, version: &str) -> Option<String> {
  let home = env
    .get("CODEX_HOME")
    .filter(|dir| !dir.trim().is_empty())
    .map_or_else(|| env.home().join(".codex"), PathBuf::from);
  let config = std::fs::read_to_string(home.join("config.toml")).ok()?;
  pin_notice(&config, version)
}

/// The notice for the Codex config text `config`.
pub(super) fn pin_notice(config: &str, version: &str) -> Option<String> {
  let table: toml::Table = toml::from_str(config).ok()?;
  let marketplace = table.get("marketplaces")?.get("toolu")?;
  let text = |key: &str| marketplace.get(key).and_then(toml::Value::as_str);
  let ours = text("source_type") == Some("git")
    && text("source")
      .is_some_and(|source| source.trim_end_matches(".git") == ORIGIN.trim_end_matches(".git"));
  if !ours {
    return None;
  }
  let want = format!("v{version}");
  match text("ref") {
    Some(pinned) if pinned == want => None,
    Some(pinned) => Some(format!(
      "the Codex marketplace toolu is pinned to {pinned}, not {want}; {FIX}"
    )),
    None => Some(format!(
      "the Codex marketplace toolu follows its default branch, so Codex asks to review toolu's hooks again after every change to it; {FIX}"
    )),
  }
}

#[cfg(test)]
#[path = "tests/codex_pin_test.rs"]
mod tests;
