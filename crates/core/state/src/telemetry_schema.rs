//! The strict telemetry schemas (`TelemetryLineSchema`, `TELEMETRY_EXTRAS` in
//! `state-schema.ts`): a persisted line is exactly one event's extras plus
//! `v` (1), `t`, `branch` and `event`, and every key is required.

use std::collections::BTreeSet;

use toolu_runtime::json::ordered::Ordered;

use crate::telemetry::TelemetryEvent;

/// A persisted telemetry line.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryLine {
  /// The event with its extras.
  pub event: TelemetryEvent,
  /// When (`isoSeconds`).
  pub t: String,
  /// The branch it was recorded on.
  pub branch: String,
}

/// The keys of an object being read; whatever is left unread is unknown.
struct Fields<'a> {
  entries: &'a [(String, Ordered)],
  read: BTreeSet<&'a str>,
}

impl<'a> Fields<'a> {
  fn of(value: &'a Ordered) -> Result<Fields<'a>, String> {
    match value {
      Ordered::Object(entries) => Ok(Fields {
        entries,
        read: BTreeSet::new(),
      }),
      Ordered::Null
      | Ordered::Bool(_)
      | Ordered::Number(_)
      | Ordered::String(_)
      | Ordered::Array(_) => Err("(root): expected object".to_owned()),
    }
  }

  fn get(&mut self, key: &'a str) -> Result<&'a Ordered, String> {
    self.read.insert(key);
    let found = self.entries.iter().find(|(name, _)| name == key);
    found
      .map(|(_, value)| value)
      .ok_or_else(|| format!("{key}: required"))
  }

  fn text(&mut self, key: &'a str) -> Result<String, String> {
    let value = self.get(key)?;
    let text = if let Ordered::String(text) = value {
      Some(text.clone())
    } else {
      None
    };
    text.ok_or_else(|| format!("{key}: expected string"))
  }

  fn maybe_text(&mut self, key: &'a str) -> Result<Option<String>, String> {
    match self.get(key)? {
      Ordered::Null => Ok(None),
      Ordered::String(text) => Ok(Some(text.clone())),
      Ordered::Bool(_) | Ordered::Number(_) | Ordered::Array(_) | Ordered::Object(_) => {
        Err(format!("{key}: expected string or null"))
      }
    }
  }

  fn number(&mut self, key: &'a str) -> Result<f64, String> {
    let value = self.get(key)?;
    let number = if let Ordered::Number(number) = value {
      number.as_f64()
    } else {
      None
    };
    number.ok_or_else(|| format!("{key}: expected number"))
  }

  fn maybe_number(&mut self, key: &'a str) -> Result<Option<f64>, String> {
    match self.get(key)? {
      Ordered::Null => Ok(None),
      Ordered::Number(number) => Ok(number.as_f64()),
      Ordered::Bool(_) | Ordered::String(_) | Ordered::Array(_) | Ordered::Object(_) => {
        Err(format!("{key}: expected number or null"))
      }
    }
  }

  /// An error naming the first key nobody read.
  fn finish(self) -> Result<(), String> {
    let unknown = self
      .entries
      .iter()
      .find(|(name, _)| !self.read.contains(name.as_str()));
    unknown.map_or(Ok(()), |(name, _)| {
      Err(format!("(root): unrecognized key \"{name}\""))
    })
  }
}

/// Reads `event`'s extras from `fields`.
fn extras(event: &str, fields: &mut Fields<'_>) -> Result<TelemetryEvent, String> {
  Ok(match event {
    "gate_fail" => TelemetryEvent::GateFail {
      file: fields.text("file")?,
      source: fields.text("source")?,
    },
    "gate_clear" => TelemetryEvent::GateClear {
      file: fields.text("file")?,
      source: fields.text("source")?,
    },
    "step_run" => TelemetryEvent::StepRun {
      step_id: fields.text("step_id")?,
      status: fields.text("status")?,
      exit_code: fields.number("exit_code")?,
      duration_s: fields.number("duration_s")?,
      attempt: fields.number("attempt")?,
    },
    "ac_coverage" => TelemetryEvent::AcCoverage {
      covered: fields.number("covered")?,
      uncovered: fields.number("uncovered")?,
    },
    "docs_attested" => TelemetryEvent::DocsAttested {
      decision: fields.text("decision")?,
    },
    "docs_nudge" => TelemetryEvent::DocsNudge,
    "push_check" => TelemetryEvent::PushCheck {
      result: fields.text("result")?,
      reason_code: fields.text("reason_code")?,
      round: fields.maybe_number("round")?,
    },
    "delegation" => TelemetryEvent::Delegation {
      model: fields.maybe_text("model")?,
      subagent_type: fields.maybe_text("subagent_type")?,
      reasoning_effort: fields.maybe_text("reasoning_effort")?,
      step_id: fields.maybe_text("step_id")?,
      step_model: fields.maybe_text("step_model")?,
    },
    other => return Err(format!("event: unknown event \"{other}\"")),
  })
}

/// `TELEMETRY_EXTRAS[event]`: exactly that event's extras, nothing else.
///
/// # Errors
/// The first offending key, as `<key>: <problem>`.
pub fn parse_telemetry_extras(event: &str, value: &Ordered) -> Result<TelemetryEvent, String> {
  let mut fields = Fields::of(value)?;
  let parsed = extras(event, &mut fields)?;
  fields.finish()?;
  Ok(parsed)
}

/// `TelemetryLineSchema`: one persisted line.
///
/// # Errors
/// The first offending key, as `<key>: <problem>`.
pub fn parse_telemetry_line(value: &Ordered) -> Result<TelemetryLine, String> {
  let mut fields = Fields::of(value)?;
  let name = fields.text("event")?;
  let event = extras(&name, &mut fields)?;
  if (fields.number("v")? - 1.0).abs() > f64::EPSILON {
    return Err("v: expected 1".to_owned());
  }
  let (t, branch) = (fields.text("t")?, fields.text("branch")?);
  fields.finish()?;
  Ok(TelemetryLine { event, t, branch })
}

#[cfg(test)]
#[path = "tests/telemetry_schema_test.rs"]
mod tests;
