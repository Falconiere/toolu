//! How engine effects reach GitHub and the trackers. The engine builds live
//! clients from its environment; tests point both at a loopback fixture.

use std::sync::Arc;

use toolu_runtime::env::Env;

use crate::tracker::{Gh, Tracker, TrackerKind, make_tracker};

/// Builds the GitHub client of one effect.
pub(crate) type MakeGh = dyn Fn(&Env) -> Result<Gh, String> + Send + Sync;

/// Builds the tracker of an epic reference: kind, reference, default repository.
pub(crate) type MakeEpicTracker =
  dyn Fn(TrackerKind, &str, &str, &Env) -> Result<Box<dyn Tracker + Sync>, String> + Send + Sync;

/// The client builders every in-process effect uses.
#[derive(Clone)]
pub(crate) struct Reach {
  /// GitHub.
  pub gh: Arc<MakeGh>,
  /// The epic's tracker.
  pub tracker: Arc<MakeEpicTracker>,
}

impl Reach {
  /// Live clients from the environment.
  pub(crate) fn live() -> Self {
    Self {
      gh: Arc::new(Gh::new),
      tracker: Arc::new(make_tracker),
    }
  }
}

/// Credentials an effect's error text must never carry.
const SECRETS: [&str; 5] = [
  "GH_TOKEN",
  "JIRA_PAT",
  "JIRA_API_TOKEN",
  "LINEAR_API_KEY",
  "TYPESAFE_API_KEY",
];

/// `text` with every credential value of `env` replaced by `[redacted]`.
pub(crate) fn redact(env: &Env, text: &str) -> String {
  SECRETS
    .iter()
    .filter_map(|name| env.get(name))
    .filter(|secret| !secret.trim().is_empty())
    .fold(text.to_owned(), |text, secret| {
      text.replace(secret, "[redacted]")
    })
}

#[cfg(test)]
#[path = "tests/reach_test.rs"]
mod tests;
