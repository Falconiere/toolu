//! Comparing a replay with its golden capture, and checking that a capture
//! says what its case says.

use serde_json::{Map, Value};

use crate::Res;
use crate::cases::{Case, Deviation, Golden, Recorded};

/// Every string in `value` as its sorted lines: order dropped, content kept.
fn lines_sorted(value: Value) -> Value {
  match value {
    Value::String(text) => {
      let mut lines: Vec<&str> = text.split('\n').collect();
      lines.sort_unstable();
      Value::Array(
        lines
          .into_iter()
          .map(|line| Value::String(line.to_owned()))
          .collect(),
      )
    }
    Value::Array(items) => Value::Array(items.into_iter().map(lines_sorted).collect()),
    Value::Object(fields) => Value::Object(
      fields
        .into_iter()
        .map(|(key, item)| (key, lines_sorted(item)))
        .collect(),
    ),
    Value::Null | Value::Bool(_) | Value::Number(_) => value,
  }
}

/// JSON text compared by content with line order dropped; other text by its sorted lines.
fn unordered(text: &str) -> Value {
  lines_sorted(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned())))
}

/// A step as compared: exact, or with line order dropped for output whose order bash never fixed.
fn comparable(step: &Recorded, loose: bool) -> Value {
  let text = |body: &str| {
    if loose {
      unordered(body)
    } else {
      Value::String(body.to_owned())
    }
  };
  let state: Map<String, Value> = step
    .state
    .iter()
    .map(|(key, body)| (key.clone(), text(body)))
    .collect();
  let mut doc = Map::new();
  doc.insert("stdout".to_owned(), text(&step.stdout));
  doc.insert("stderr".to_owned(), Value::String(step.stderr.clone()));
  doc.insert("exitCode".to_owned(), Value::from(step.exit_code));
  doc.insert("state".to_owned(), Value::Object(state));
  Value::Object(doc)
}

fn clip(value: &Value) -> String {
  let text = value.to_string();
  match text.char_indices().nth(1500) {
    Some((end, _)) => format!(
      "{}... ({} bytes)",
      text.get(..end).unwrap_or(""),
      text.len()
    ),
    None => text,
  }
}

/// Where two comparable steps first differ, as text.
fn describe(index: usize, expected: &Value, actual: &Value) -> String {
  for field in ["stdout", "stderr", "exitCode"] {
    if expected.get(field) != actual.get(field) {
      let (want, got) = (expected.get(field), actual.get(field));
      return format!(
        "step {index} {field}:\n  golden: {}\n  actual: {}",
        show(want),
        show(got)
      );
    }
  }
  let empty = Map::new();
  let want = expected
    .get("state")
    .and_then(Value::as_object)
    .unwrap_or(&empty);
  let got = actual
    .get("state")
    .and_then(Value::as_object)
    .unwrap_or(&empty);
  for key in want.keys().chain(got.keys()) {
    if want.get(key) != got.get(key) {
      let (a, b) = (want.get(key), got.get(key));
      return format!(
        "step {index} state {key}:\n  golden: {}\n  actual: {}",
        show(a),
        show(b)
      );
    }
  }
  format!("step {index} differs")
}

fn show(value: Option<&Value>) -> String {
  value.map_or_else(|| "<absent>".to_owned(), clip)
}

/// `Ok` when `actual` equals `expected` step for step.
pub(crate) fn matches(
  case: &Case,
  expected: &[Recorded],
  actual: &[Recorded],
) -> Result<(), String> {
  if expected.len() != actual.len() {
    return Err(format!(
      "{} steps ran, the golden has {}",
      actual.len(),
      expected.len()
    ));
  }
  for (index, (want, got)) in expected.iter().zip(actual).enumerate() {
    let (want, got) = (
      comparable(want, case.unordered),
      comparable(got, case.unordered),
    );
    if want != got {
      return Err(describe(index, &want, &got));
    }
  }
  Ok(())
}

/// `Ok` when the port shows what the deviation names and no longer prints the capture.
pub(crate) fn deviates(
  deviation: &Deviation,
  expected: &[Recorded],
  actual: &[Recorded],
) -> Result<(), String> {
  let last = actual.last().map_or("", |step| step.stdout.as_str());
  if let Some(missing) = deviation
    .contains
    .iter()
    .find(|text| !last.contains(text.as_str()))
  {
    return Err(format!("deviation: stdout lacks {missing:?}: {last:?}"));
  }
  if expected.last().is_some_and(|step| step.stdout == last) {
    return Err("deviation: stdout still equals the golden's".to_owned());
  }
  Ok(())
}

/// The decision class of one hook result, as the #259 parity suite classifies it.
fn outcome(step: &Recorded) -> Res<&'static str> {
  if step.exit_code == 2 {
    return Ok("exit2");
  }
  if step.stdout.trim().is_empty() {
    return Ok("silent");
  }
  let out: Value = serde_json::from_str(&step.stdout)?;
  let block = out.get("decision").and_then(Value::as_str) == Some("block");
  Ok(if block { "block" } else { "advisory" })
}

/// The capture of `case` is complete and says what the bats suites said: each
/// step exited 0 and the last one's decision class and text match the case.
pub(crate) fn capture_meets(case: &Case, steps: &[Recorded]) -> Res<()> {
  if steps.len() != case.steps.len() {
    return Err(
      format!(
        "{} captured steps, {} in the case",
        steps.len(),
        case.steps.len()
      )
      .into(),
    );
  }
  if let Some(step) = steps.iter().find(|step| step.exit_code != 0) {
    return Err(format!("a captured step exited {}", step.exit_code).into());
  }
  let last = steps.last().ok_or("no steps")?;
  let class = outcome(last)?;
  if class != case.expect {
    return Err(format!("captured outcome {class}, the case expects {}", case.expect).into());
  }
  for text in &case.contains {
    if !last.stdout.contains(text.as_str()) {
      return Err(format!("captured stdout lacks {text:?}").into());
    }
  }
  match case
    .absent
    .iter()
    .find(|text| last.stdout.contains(text.as_str()))
  {
    Some(text) => Err(format!("captured stdout holds {text:?}").into()),
    None => Ok(()),
  }
}

/// The golden holds exactly the cases' keys, each once.
pub(crate) fn same_keys(keys: &[String], golden: &Golden) -> Res<()> {
  let mut sorted = keys.to_vec();
  sorted.sort();
  let before = sorted.len();
  sorted.dedup();
  if sorted.len() != before {
    return Err("two cases share a golden key".into());
  }
  let captured: Vec<&String> = golden.cases.keys().collect();
  if captured != sorted.iter().collect::<Vec<_>>() {
    return Err(
      format!(
        "golden keys differ from the cases: {} against {}",
        captured.len(),
        sorted.len()
      )
      .into(),
    );
  }
  Ok(())
}
