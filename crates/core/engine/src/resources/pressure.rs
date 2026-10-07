//! Replayable sustained-pressure decisions (`packages/toolu-core/src/resources/pressure.ts`):
//! a machine sample, its strict schema, and the hold that refuses new work
//! after a minute of load or memory pressure and lifts after two good minutes.

use toolu_runtime::json::ordered::Ordered;

use super::store::safe_integer;
use crate::ledger::jq::number;

/// How old a sample may be before a new one is taken.
pub const PRESSURE_SAMPLE_MS: f64 = 30_000.0;
/// How long pressure must last before new work is held.
pub const PRESSURE_HOLD_MS: f64 = 60_000.0;
/// How long the machine must be calm before the hold lifts.
pub const PRESSURE_RECOVERY_MS: f64 = 120_000.0;

/// `/proc/stat` CPU ticks: the total and the steal share.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ticks {
  /// All ticks of the first eight fields.
  pub total: f64,
  /// Steal ticks.
  pub steal: f64,
}

/// One machine sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResourceSample {
  /// When, in epoch milliseconds.
  pub at: f64,
  /// Logical CPUs.
  pub cpus: f64,
  /// The one-minute load average.
  pub load: f64,
  /// Reclaimable memory.
  pub available_bytes: f64,
  /// All memory.
  pub total_bytes: f64,
  /// The share of CPU time stolen since the previous sample, when known.
  pub steal: Option<f64>,
  /// The raw tick counters, on Linux.
  pub ticks: Option<Ticks>,
}

/// The pressure state persisted in `state.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pressure {
  /// Whether new work is held.
  pub held: bool,
  /// Since when the machine has been under pressure.
  pub bad_since: Option<f64>,
  /// Since when the machine has been calm.
  pub good_since: Option<f64>,
  /// The latest sample.
  pub sample: ResourceSample,
  /// Why work is held.
  pub reason: Option<String>,
}

fn finite(value: Option<&Ordered>) -> Option<f64> {
  match value {
    Some(Ordered::Number(n)) => n.as_f64().filter(|f| f.is_finite()),
    _ => None,
  }
}

/// A safe integer of at least `min`, as a number.
fn whole_at_least(value: Option<&Ordered>, min: i64) -> Option<f64> {
  value.and_then(safe_integer).filter(|n| *n >= min)?;
  finite(value)
}

fn ticks_of(value: &Ordered) -> Option<Ticks> {
  Some(Ticks {
    total: whole_at_least(value.get("total"), 0)?,
    steal: whole_at_least(value.get("steal"), 0)?,
  })
}

/// `isSample`, read into a typed sample.
pub fn sample_of(value: &Ordered) -> Option<ResourceSample> {
  let Ordered::Object(_) = value else {
    return None;
  };
  let at = finite(value.get("at")).filter(|at| *at >= 0.0)?;
  let cpus = whole_at_least(value.get("cpus"), 1)?;
  let load = finite(value.get("load")).filter(|load| *load >= 0.0)?;
  let total_bytes = finite(value.get("totalBytes")).filter(|total| *total > 0.0)?;
  let available_bytes =
    finite(value.get("availableBytes")).filter(|avail| *avail >= 0.0 && *avail <= total_bytes)?;
  let steal = match value.get("steal") {
    Some(Ordered::Null) => None,
    other => Some(finite(other).filter(|steal| (0.0..=1.0).contains(steal))?),
  };
  let ticks = match value.get("ticks") {
    None => None,
    Some(raw) => Some(ticks_of(raw)?),
  };
  Some(ResourceSample {
    at,
    cpus,
    load,
    available_bytes,
    total_bytes,
    steal,
    ticks,
  })
}

/// `isPressure`, read into a typed pressure.
pub fn pressure_of(value: &Ordered) -> Option<Pressure> {
  let Ordered::Object(_) = value else {
    return None;
  };
  let sample = sample_of(value.get("sample")?)?;
  let since = |key: &str| match value.get(key) {
    Some(Ordered::Null) => Some(None),
    other => finite(other).filter(|at| *at <= sample.at).map(Some),
  };
  let (bad_since, good_since) = (since("badSince")?, since("goodSince")?);
  let Some(Ordered::Bool(held)) = value.get("held") else {
    return None;
  };
  let reason = match (held, value.get("reason")) {
    (true, Some(Ordered::String(reason))) => Some(reason.clone()),
    (false, Some(Ordered::Null)) => None,
    _ => return None,
  };
  Some(Pressure {
    held: *held,
    bad_since,
    good_since,
    sample,
    reason,
  })
}

/// `isPressure`.
pub fn is_pressure(value: &Ordered) -> bool {
  pressure_of(value).is_some()
}

fn optional(value: Option<f64>) -> Ordered {
  value.map_or(Ordered::Null, number)
}

impl ResourceSample {
  /// The sample as TypeScript writes it.
  pub fn to_ordered(&self) -> Ordered {
    let mut entries = vec![
      ("at".to_owned(), number(self.at)),
      ("cpus".to_owned(), number(self.cpus)),
      ("load".to_owned(), number(self.load)),
      ("availableBytes".to_owned(), number(self.available_bytes)),
      ("totalBytes".to_owned(), number(self.total_bytes)),
      ("steal".to_owned(), optional(self.steal)),
    ];
    if let Some(ticks) = self.ticks {
      let ticks = vec![
        ("total".to_owned(), number(ticks.total)),
        ("steal".to_owned(), number(ticks.steal)),
      ];
      entries.push(("ticks".to_owned(), Ordered::Object(ticks)));
    }
    Ordered::Object(entries)
  }
}

impl Pressure {
  /// The pressure as TypeScript writes it.
  pub fn to_ordered(&self) -> Ordered {
    Ordered::Object(vec![
      ("held".to_owned(), Ordered::Bool(self.held)),
      ("badSince".to_owned(), optional(self.bad_since)),
      ("goodSince".to_owned(), optional(self.good_since)),
      ("sample".to_owned(), self.sample.to_ordered()),
      (
        "reason".to_owned(),
        self.reason.clone().map_or(Ordered::Null, Ordered::String),
      ),
    ])
  }
}

/// `advancePressure(previous, sample)`: the next pressure state.
///
/// # Errors
/// An invalid sample, or one older than the previous.
pub fn advance_pressure(
  previous: Option<&Pressure>,
  sample: &ResourceSample,
) -> Result<Pressure, String> {
  if sample_of(&sample.to_ordered()).is_none() {
    return Err("invalid resource pressure sample".to_owned());
  }
  if previous.is_some_and(|previous| sample.at < previous.sample.at) {
    return Err("resource pressure sample moved backwards".to_owned());
  }
  let steal = sample.steal.unwrap_or(0.0);
  let memory = sample.available_bytes / sample.total_bytes;
  let effective = (sample.cpus * (1.0 - steal)).max(0.1);
  let bad = sample.load > effective * 1.5 || memory < 0.1 || steal > 0.25;
  let good = sample.load < effective * 0.8 && memory > 0.2 && steal < 0.1;
  let bad_since = bad.then(|| previous.and_then(|p| p.bad_since).unwrap_or(sample.at));
  let good_since = good.then(|| previous.and_then(|p| p.good_since).unwrap_or(sample.at));
  let mut held = previous.is_some_and(|p| p.held);
  if bad_since.is_some_and(|since| sample.at - since >= PRESSURE_HOLD_MS) {
    held = true;
  }
  if good_since.is_some_and(|since| sample.at - since >= PRESSURE_RECOVERY_MS) {
    held = false;
  }
  Ok(Pressure {
    held,
    bad_since,
    good_since,
    sample: *sample,
    reason: held.then(|| "sustained CPU/load or memory pressure".to_owned()),
  })
}

#[cfg(test)]
#[path = "tests/pressure_test.rs"]
mod tests;
