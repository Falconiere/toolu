//! This process's own resource use from `getrusage(RUSAGE_SELF)`: the peak
//! resident set and the CPU time spent so far, for the epic engine's
//! `engine-usage` journal samples (#451).

use nix::sys::resource::{UsageWho, getrusage};
use nix::sys::time::TimeValLike as _;

/// Peak memory and CPU time of the calling process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
  /// The largest resident set so far, in bytes on every platform.
  pub max_rss_bytes: u64,
  /// User plus system CPU time, in milliseconds.
  pub cpu_ms: u64,
}

/// The calling process's usage.
///
/// # Errors
/// `getrusage` fails.
pub fn self_usage() -> Result<Usage, String> {
  let usage = getrusage(UsageWho::RUSAGE_SELF).map_err(|err| format!("getrusage: {err}"))?;
  let cpu = usage
    .user_time()
    .num_milliseconds()
    .saturating_add(usage.system_time().num_milliseconds());
  Ok(Usage {
    max_rss_bytes: to_bytes(usage.max_rss()),
    cpu_ms: u64::try_from(cpu).unwrap_or(0),
  })
}

/// `ru_maxrss` is bytes on macOS and kibibytes elsewhere.
fn to_bytes(max_rss: i64) -> u64 {
  let value = u64::try_from(max_rss).unwrap_or(0);
  if cfg!(target_os = "macos") {
    value
  } else {
    value.saturating_mul(1024)
  }
}

#[cfg(test)]
#[path = "tests/usage_test.rs"]
mod tests;
