//! Telemetry retention (`sweepTelemetry` in `state-sweeper.ts`): each
//! `telemetry/*.jsonl` file keeps the lines `jq -c 'select((.t // "") >=
//! $cutoff)'` keeps, is deleted when none is left, and is left as it is when
//! any line is not JSON (jq would fail on it).

use std::path::Path;
use std::time::Duration;

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use crate::ctx::StateCtx;
use crate::io::write_atomic;
use crate::sweeper::{glob_files, real_dir, remove};
use crate::time::iso_seconds;

const DAY: Duration = Duration::from_hours(24);

/// The lines of `content` at or after `cutoff`, as `jq -c` prints them, or
/// `None` when jq would fail (bad JSON, or a value neither an object nor null).
pub fn kept_telemetry_lines(content: &str, cutoff: &str) -> Option<Vec<String>> {
  let mut kept = Vec::new();
  for line in content.split('\n').filter(|line| !line.trim().is_empty()) {
    let value = Ordered::parse(line).ok()?;
    let t = match &value {
      Ordered::Null => continue,
      Ordered::Object(_) => value.get("t"),
      Ordered::Bool(_) | Ordered::Number(_) | Ordered::String(_) | Ordered::Array(_) => {
        return None;
      }
    };
    // jq order: null, false, true, numbers < strings < arrays and objects.
    let keep = match t {
      Some(Ordered::String(t)) => t.as_str() >= cutoff,
      Some(Ordered::Array(_) | Ordered::Object(_)) => true,
      None | Some(Ordered::Null | Ordered::Bool(_) | Ordered::Number(_)) => false,
    };
    if keep {
      kept.push(jq_text(&value, false));
    }
  }
  Some(kept)
}

/// Trims every telemetry file in `dir` to the last `retention_days` days.
pub(crate) fn sweep_telemetry(ctx: &mut StateCtx, dir: &Path, retention_days: u64) {
  let window = DAY.saturating_mul(u32::try_from(retention_days).unwrap_or(u32::MAX));
  let cutoff = iso_seconds(
    ctx
      .now()
      .checked_sub(window)
      .unwrap_or(std::time::UNIX_EPOCH),
  );
  if !real_dir(dir) {
    return;
  }
  for file in glob_files(dir, ".jsonl") {
    // Decoded as TypeScript decodes it: invalid UTF-8 becomes U+FFFD.
    let content = match std::fs::read(&file) {
      Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
      Err(err) => {
        ctx.warnings.push(format!(
          "toolu-sweep: could not read {}: {err}",
          file.display()
        ));
        continue;
      }
    };
    let Some(kept) = kept_telemetry_lines(&content, &cutoff) else {
      continue;
    };
    if kept.is_empty() {
      remove(&file, &mut ctx.warnings);
    } else if !write_atomic(&file, &format!("{}\n", kept.join("\n"))) {
      ctx
        .warnings
        .push(format!("toolu-sweep: could not trim {}", file.display()));
    }
  }
}

#[cfg(test)]
#[path = "tests/sweep_telemetry_test.rs"]
mod tests;
