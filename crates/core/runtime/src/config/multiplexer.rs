//! The global `multiplexer` config key: which pane manager runs worker agents.

use super::load::LoadedConfig;

/// The configured pane manager for worker agents.
///
/// `Auto` prefers herdr and falls back to plain tmux when herdr is not
/// reachable, so a machine with neither installed still gets a clear warning
/// rather than a silent choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Multiplexer {
  /// Prefer herdr; fall back to tmux when herdr is unreachable.
  #[default]
  Auto,
  /// herdr only.
  Herdr,
  /// herdr-core terminals, controlled by `hc`.
  HerdrApp,
  /// tmux only.
  Tmux,
}

impl Multiplexer {
  /// The top-level `multiplexer` key, defaulting to [`Multiplexer::Auto`].
  ///
  /// # Errors
  /// `multiplexer must be auto, herdr or tmux` for any other value.
  pub fn parse(config: &LoadedConfig) -> Result<Multiplexer, String> {
    let Some(value) = config.data.get("multiplexer") else {
      return Ok(Multiplexer::Auto);
    };
    match value.as_str() {
      Some("auto") => Ok(Multiplexer::Auto),
      Some("herdr") => Ok(Multiplexer::Herdr),
      Some("herdr-app") => Ok(Multiplexer::HerdrApp),
      Some("tmux") => Ok(Multiplexer::Tmux),
      _ => Err("multiplexer must be auto, herdr, herdr-app or tmux".to_owned()),
    }
  }

  /// The spelling the config file and `TOOLU_EPIC_MULTIPLEXER` both use.
  #[must_use]
  pub fn as_str(self) -> &'static str {
    match self {
      Multiplexer::Auto => "auto",
      Multiplexer::Herdr => "herdr",
      Multiplexer::HerdrApp => "herdr-app",
      Multiplexer::Tmux => "tmux",
    }
  }
}

#[cfg(test)]
#[path = "tests/multiplexer_test.rs"]
mod tests;
