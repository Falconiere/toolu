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
/// `Auto` prefers herdr, then tmux. A pinned backend is returned
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

/// Resolve config and the explicit environment override without accepting typos.
///
/// # Errors
/// The multiplexer value is invalid.
pub fn configured(env: &Env) -> Result<Backend, String> {
  let configured = match env.get("TOOLU_EPIC_MULTIPLEXER") {
    Some("auto") => Multiplexer::Auto,
    Some("herdr") => Multiplexer::Herdr,
    Some("herdr-app") => {
      return Err("TOOLU_EPIC_MULTIPLEXER herdr-app is unsupported; choose auto, herdr or tmux; recover retired runs with a pre-removal Toolu binary".into());
    }
    Some("tmux") => Multiplexer::Tmux,
    Some(_) => return Err("TOOLU_EPIC_MULTIPLEXER must be auto, herdr or tmux".into()),
    None => {
      let roots = toolu_runtime::host::roots::Roots::new(env.clone(), None);
      Multiplexer::parse(&toolu_runtime::config::load::load(&roots, None))?
    }
  };
  Ok(resolve(configured, env))
}

#[cfg(test)]
#[path = "tests/backend_test.rs"]
mod tests;
