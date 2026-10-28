use std::path::Path;
use std::time::Duration;

use crate::budget::Effort;
use crate::memory::Memory;
use crate::prepass::PrepassResult;
use crate::rules::Bundle;
use crate::settings::Settings;
use crate::shape::Shaped;

use super::{Record, Replay};

/// What one run reviews.
#[derive(Debug, Clone, Copy)]
pub struct Input<'a> {
  /// The repository top level.
  pub root: &'a Path,
  /// The committed diff.
  pub shaped: &'a Shaped,
  /// Base-ref conventions and the prompt override.
  pub rules: &'a Bundle,
  /// Trusted scanner findings and secret spans.
  pub prepass: &'a PrepassResult,
  /// Earlier rounds' findings and dismissals.
  pub memory: &'a Memory,
  /// The `codeReview` settings: enablement and configured effort.
  pub settings: &'a Settings,
  /// `--effort`, when given.
  pub flag: Option<Effort>,
}

/// How one run calls out; production uses the defaults of each client.
#[derive(Clone, Default)]
pub struct Options<'a> {
  /// The review model's endpoint.
  pub openrouter: toolu_openrouter::Config,
  /// Jev's endpoint; its default is replaced by `OpenRouter`'s System One.
  pub jev: Option<toolu_jev_client::Config>,
  /// `true` skips the Jev stage: the baseline arm of #670.
  pub skip_judge: bool,
  /// Receives every raw exchange (#670).
  pub recorder: Option<&'a dyn Fn(&Record<'_>)>,
  /// Answers calls from recorded responses before any network call, and fails a
  /// miss closed instead of sending it (#670).
  pub replay: Option<Replay<'a>>,
  /// With a `replay` lookup, send a request it does not hold instead of failing it
  /// closed: a live run that records into the store and reuses its own entries.
  pub live_on_miss: bool,
  /// Overrides the level's review-call deadline.
  pub review_deadline: Option<Duration>,
  /// Overrides the level's shared judge deadline.
  pub judge_deadline: Option<Duration>,
}
