//! When a throttled reply says it resets: `Retry-After` (seconds or an HTTP
//! date), then Linear's epoch-millisecond or GitHub's epoch-second
//! `x-ratelimit-*reset` once the matching remaining count is `0`.

use std::time::UNIX_EPOCH;

const MONTHS: [&str; 12] = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The reset in Unix milliseconds, from headers looked up by lowercase name.
pub(crate) fn reset_from_headers<'a>(
  header: impl Fn(&str) -> Option<&'a str>,
  now_ms: u64,
) -> Option<u64> {
  if let Some(retry_after) = header("retry-after") {
    let text = retry_after.trim();
    if let Ok(seconds) = text.parse::<f64>()
      && seconds.is_finite()
      && seconds >= 0.0
    {
      return Some(now_ms.saturating_add(millis(seconds)));
    }
    if let Some(at) = http_date_ms(text) {
      return Some(at);
    }
  }
  let raw = header("x-ratelimit-requests-reset").or_else(|| header("x-ratelimit-reset"))?;
  let remaining =
    header("x-ratelimit-requests-remaining").or_else(|| header("x-ratelimit-remaining"));
  if remaining.map(str::trim) != Some("0") {
    return None;
  }
  let value = raw.trim().parse::<u64>().ok()?;
  Some(if value > 1_000_000_000_000 {
    value
  } else {
    value.saturating_mul(1000)
  })
}

fn millis(seconds: f64) -> u64 {
  let ms = (seconds * 1000.0).round();
  if ms >= 1.8e19 {
    return u64::MAX;
  }
  format!("{ms:.0}").parse().unwrap_or(0)
}

/// An IMF-fixdate (`Wed, 21 Oct 2015 07:28:00 GMT`) in Unix milliseconds.
fn http_date_ms(text: &str) -> Option<u64> {
  let rest = text.split_once(", ").map_or(text, |(_, rest)| rest);
  let mut parts = rest.split_whitespace();
  let day: u32 = parts.next()?.parse().ok()?;
  let month_name = parts.next()?;
  let month = MONTHS.iter().position(|name| *name == month_name)?;
  let year: u32 = parts.next()?.parse().ok()?;
  let clock = parts.next()?;
  if parts.next() != Some("GMT") || parts.next().is_some() {
    return None;
  }
  let iso = format!("{year:04}-{:02}-{day:02}T{clock}Z", month + 1);
  let at = toolu_state::time::parse_iso(&iso)?;
  at.duration_since(UNIX_EPOCH)
    .ok()
    .and_then(|since| u64::try_from(since.as_millis()).ok())
}

#[cfg(test)]
#[path = "tests/reset_test.rs"]
mod tests;
