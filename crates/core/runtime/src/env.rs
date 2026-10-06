//! `Env`: the environment a command resolves its host and roots against, as an
//! explicit snapshot (`HostEnv` in `packages/toolu-core/src/host/host-name.ts`).
//! Only this crate reads `std::env` (rule 14).

use std::collections::BTreeMap;
use std::path::PathBuf;

/// A snapshot of environment variables. An empty value counts as unset, like
/// bash `${VAR:-}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env(BTreeMap<String, String>);

impl Env {
  /// The process environment; a non-UTF-8 name or value is replaced lossily.
  pub fn process() -> Env {
    let vars = std::env::vars_os().map(|(key, value)| {
      let key = key.to_string_lossy().into_owned();
      (key, value.to_string_lossy().into_owned())
    });
    Env(vars.collect())
  }

  /// An environment of exactly `pairs`.
  pub fn from_pairs<I, K, V>(pairs: I) -> Env
  where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
  {
    let vars = pairs
      .into_iter()
      .map(|(key, value)| (key.into(), value.into()));
    Env(vars.collect())
  }

  /// `key`'s value when it is set and not empty.
  pub fn get(&self, key: &str) -> Option<&str> {
    self
      .0
      .get(key)
      .map(String::as_str)
      .filter(|value| !value.is_empty())
  }

  /// Every variable, empty ones included: what a child process receives.
  pub fn vars(&self) -> impl Iterator<Item = (&str, &str)> {
    self
      .0
      .iter()
      .map(|(key, value)| (key.as_str(), value.as_str()))
  }

  /// This snapshot with `key` set to `value`.
  #[must_use]
  pub fn with(mut self, key: &str, value: &str) -> Env {
    self.0.insert(key.to_owned(), value.to_owned());
    self
  }

  /// `HOME`, else the operating system's home directory (Node's `homedir()`),
  /// else the empty path.
  pub fn home(&self) -> PathBuf {
    match self.get("HOME") {
      Some(home) => PathBuf::from(home),
      None => std::env::home_dir().unwrap_or_default(),
    }
  }
}

#[cfg(test)]
#[path = "tests/env_test.rs"]
mod tests;
