//! Epic metrics (#451): what an epic run cost, folded from the engine's
//! journal, the same figures derived from the TypeScript watcher's state
//! directories as a baseline, and the efficiency budgets judged on them.
//!
//! - `document`: the schema-1 metrics document (`toolu epic metrics`,
//!   `/api/v1/metrics`);
//! - `budgets`: the budget table;
//! - `baseline`: the TypeScript-era figures (`toolu epic baseline`);
//! - `text`: the human table beside the committed baseline;
//! - `stats`, `scope`, `timeline`, `ready`, `counts`, `github`: the fold.

pub(crate) mod baseline;
mod budgets;
mod counts;
mod document;
mod github;
mod ready;
mod scope;
mod stats;
mod text;
mod timeline;

use std::time::Duration;

use toolu_state::time::{epoch_millis, parse_iso};

pub(crate) use budgets::rows as budget_rows;
pub(crate) use document::document;
pub(crate) use text::render;

/// What a metrics document covers and when it is computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Query {
  /// One epic, or every epic.
  pub epic: Option<String>,
  /// Records before this Unix millisecond are left out.
  pub since_ms: Option<u64>,
  /// Open phases count up to this Unix millisecond.
  pub now_ms: u64,
}

/// `YYYY-MM-DD` (UTC midnight) or an ISO-8601 UTC timestamp, as Unix ms.
///
/// # Errors
/// The text is neither.
pub(crate) fn parse_since(text: &str) -> Result<u64, String> {
  let full = if text.len() == 10 {
    format!("{text}T00:00:00Z")
  } else {
    text.to_owned()
  };
  parse_iso(&full)
    .map(epoch_millis)
    .ok_or_else(|| format!("bad date {text:?}: want YYYY-MM-DD or an ISO-8601 UTC time"))
}

/// Milliseconds as a short human duration.
pub(crate) fn human(ms: u64) -> String {
  let duration = Duration::from_millis(ms);
  let secs = duration.as_secs();
  match secs {
    0..60 => format!("{secs} s"),
    60..3_600 => format!("{} min", secs / 60),
    _ => format!("{} h {} min", secs / 3_600, secs % 3_600 / 60),
  }
}

#[cfg(test)]
#[path = "tests/metrics_test.rs"]
mod tests;
