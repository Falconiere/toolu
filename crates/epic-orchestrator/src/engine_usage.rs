//! The engine's own resource samples (#451): an `engine-usage` journal line
//! with its peak resident memory and CPU time at start, every 15 minutes from
//! the 30 s tick, and at a clean stop.

use serde_json::json;
use toolu_runtime::process::usage::self_usage;

use crate::journal::{self, Record};
use crate::server::Engine;

/// Time between two `sample` lines.
const SAMPLE_MS: u64 = 15 * 60 * 1000;

/// Journal one sample for `event` (`start`, `sample` or `stop`).
///
/// # Errors
/// `getrusage` or the journal fails.
pub(crate) fn sample(engine: &mut Engine, event: &str) -> Result<(), String> {
  let usage = self_usage()?;
  let note = json!({"event": event, "max_rss_bytes": usage.max_rss_bytes, "cpu_ms": usage.cpu_ms});
  let record = Record::new("action", "engine-usage", "", "", &note.to_string());
  journal::append(
    &engine.paths.journal_dir(),
    &engine.paths.journal_lock(),
    &record,
    engine.now,
  )?;
  engine.world.usage_at_ms = engine.world.now_ms;
  Ok(())
}

/// Journal a `sample` when 15 minutes passed since the last line.
///
/// # Errors
/// As [`sample`].
pub(crate) fn on_tick(engine: &mut Engine) -> Result<(), String> {
  if engine.world.now_ms.saturating_sub(engine.world.usage_at_ms) < SAMPLE_MS {
    return Ok(());
  }
  sample(engine, "sample")
}

#[cfg(test)]
#[path = "tests/engine_usage_test.rs"]
mod tests;
