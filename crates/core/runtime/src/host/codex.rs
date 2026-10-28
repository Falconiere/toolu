//! Codex marketplace pin status from `config.toml` (#626).

use std::path::PathBuf;

use crate::env::Env;

/// Where toolu's own marketplace comes from, as Codex records it.
const ORIGIN: &str = "https://github.com/Falconiere/toolu.git";

/// The fix every notice names.
const FIX: &str = "toolu plugins update --host codex";

/// Notice for `$CODEX_HOME/config.toml` (else `~/.codex/config.toml`);
/// `None` when pinned to `v<version>`, absent, unreadable, or not toolu's marketplace.
pub fn marketplace_pin_notice_from_env(env: &Env, version: &str) -> Option<String> {
  let home = env
    .get("CODEX_HOME")
    .filter(|dir| !dir.trim().is_empty())
    .map_or_else(|| env.home().join(".codex"), PathBuf::from);
  let config = std::fs::read_to_string(home.join("config.toml")).ok()?;
  marketplace_pin_notice(&config, version)
}

/// Notice for the Codex config text `config`.
pub fn marketplace_pin_notice(config: &str, version: &str) -> Option<String> {
  let table: toml::Table = toml::from_str(config).ok()?;
  let marketplace = table.get("marketplaces")?.get("toolu")?;
  let text = |key: &str| marketplace.get(key).and_then(toml::Value::as_str);
  let bare = |url: &str| {
    url
      .trim_end_matches('/')
      .trim_end_matches(".git")
      .to_ascii_lowercase()
  };
  let ours = text("source_type") == Some("git")
    && text("source").is_some_and(|source| bare(source) == bare(ORIGIN));
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
#[path = "tests/codex_test.rs"]
mod tests;
