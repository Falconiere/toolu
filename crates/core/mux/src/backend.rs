//! The concrete pane manager and the resolution of the configured choice.

use toolu_runtime::config::multiplexer::Multiplexer;
use toolu_runtime::env::Env;

use crate::detect::{herdr_reachable, tmux_available};

/// The concrete pane manager that runs worker agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
  /// herdr, the tmux-based agent manager.
  Herdr,
  /// Plain tmux.
  Tmux,
}

impl Backend {
  /// The backend's name, as the config and env spell it.
  #[must_use]
  pub fn name(self) -> &'static str {
    match self {
      Backend::Herdr => "herdr",
      Backend::Tmux => "tmux",
    }
  }
}

/// Resolve the configured [`Multiplexer`] to a concrete [`Backend`] for `env`.
///
/// `Auto` prefers herdr and falls back to tmux. A pinned backend is returned
/// even when it is not reachable, so the caller can report the outage rather
/// than silently switching.
#[must_use]
pub fn resolve(configured: Multiplexer, env: &Env) -> Backend {
  match configured {
    Multiplexer::Herdr => Backend::Herdr,
    Multiplexer::Tmux => Backend::Tmux,
    Multiplexer::Auto => {
      if herdr_reachable(env) {
        Backend::Herdr
      } else {
        Backend::Tmux
      }
    }
  }
}

/// Whether the resolved [`Backend`] is actually usable for `env`.
///
/// A pinned backend that is not reachable is reported as unavailable, so the
/// caller can warn the user instead of failing later mid-launch.
#[must_use]
pub fn available(backend: Backend, env: &Env) -> bool {
  match backend {
    Backend::Herdr => herdr_reachable(env),
    Backend::Tmux => tmux_available(env),
  }
}

#[cfg(test)]
#[path = "tests/backend_test.rs"]
mod tests;
