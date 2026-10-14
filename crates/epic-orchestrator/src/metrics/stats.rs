//! Percentiles, shares and decimal figures in integer arithmetic: values are
//! milliseconds, percentiles interpolate linearly between ranks and round
//! down to the millisecond, and a share keeps four decimals.

use serde_json::{Value, json};

/// Linear-interpolated percentile `p` (0–100) of sorted `values`.
fn percentile(sorted: &[u64], p: u64) -> Option<u64> {
  let last = u64::try_from(sorted.len().checked_sub(1)?).ok()?;
  let scaled = p.saturating_mul(last);
  let low = scaled / 100;
  let part = scaled % 100;
  let at = |rank: u64| {
    usize::try_from(rank)
      .ok()
      .and_then(|rank| sorted.get(rank).copied())
  };
  let below = at(low)?;
  let above = at(low.saturating_add(u64::from(part > 0)))?;
  Some(below.saturating_add(above.saturating_sub(below).saturating_mul(part) / 100))
}

/// `{"count", "p50", "p90", "max"}` of `values`.
pub(crate) fn pct(values: &[u64]) -> Value {
  let mut sorted = values.to_vec();
  sorted.sort_unstable();
  json!({
    "count": sorted.len(),
    "p50": percentile(&sorted, 50),
    "p90": percentile(&sorted, 90),
    "max": sorted.last(),
  })
}

/// The sum of `values`, saturating instead of overflowing.
pub(crate) fn total<'a>(values: impl IntoIterator<Item = &'a u64>) -> u64 {
  values
    .into_iter()
    .fold(0, |sum, value| sum.saturating_add(*value))
}

/// `part / total` with four decimals, or `null` when `total` is 0.
pub(crate) fn share(part: u64, total: u64) -> Value {
  if total == 0 {
    return Value::Null;
  }
  let tenths_of_mille = part.saturating_mul(10_000) / total;
  decimal(tenths_of_mille, 10_000)
}

/// `numerator / denominator` as a JSON number, exact for the small integers
/// this module produces.
pub(crate) fn decimal(numerator: u64, denominator: u32) -> Value {
  let Ok(numerator) = u32::try_from(numerator) else {
    return Value::Null;
  };
  json!(f64::from(numerator) / f64::from(denominator))
}

#[cfg(test)]
#[path = "tests/stats_test.rs"]
mod tests;
