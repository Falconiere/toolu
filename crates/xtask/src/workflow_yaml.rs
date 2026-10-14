//! Lenient accessors over a parsed workflow, the way the workflow contract reads
//! it: a missing key, or a value of another type, reads as empty.

use serde_json::Value;

/// The value under `keys`, or `null` when any step is missing or not an object.
pub(crate) fn at<'v>(value: &'v Value, keys: &[&str]) -> &'v Value {
  keys
    .iter()
    .try_fold(value, |current, key| current.as_object()?.get(*key))
    .unwrap_or(&Value::Null)
}

/// `value` when it is a string, else `""`.
pub(crate) fn text(value: &Value) -> &str {
  value.as_str().unwrap_or("")
}

/// `value`'s items when it is an array, else none.
pub(crate) fn items(value: &Value) -> &[Value] {
  value.as_array().map_or(&[], Vec::as_slice)
}

/// Whether `value` is an array holding the string `item`.
pub(crate) fn lists(value: &Value, item: &str) -> bool {
  items(value)
    .iter()
    .any(|entry| entry.as_str() == Some(item))
}

/// A job's steps.
pub(crate) fn steps(job: &Value) -> &[Value] {
  items(at(job, &["steps"]))
}

/// Every `run` script of a job's steps, one after another.
pub(crate) fn runs(job: &Value) -> String {
  steps(job)
    .iter()
    .map(|step| text(at(step, &["run"])))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Whether `value` is a string containing `needle`.
pub(crate) fn includes(value: &Value, needle: &str) -> bool {
  text(value).contains(needle)
}

/// The first step of `job` that `pick` accepts, else `null`.
pub(crate) fn step_where(job: &Value, pick: impl Fn(&Value) -> bool) -> &Value {
  steps(job)
    .iter()
    .find(|step| pick(step))
    .unwrap_or(&Value::Null)
}

/// The findings of one contract run.
#[derive(Debug, Default)]
pub(crate) struct Findings(pub(crate) Vec<String>);

impl Findings {
  /// Record `message` unless `holds`.
  pub(crate) fn need(&mut self, holds: bool, message: &str) {
    if !holds {
      self.0.push(message.to_owned());
    }
  }
}

#[cfg(test)]
#[path = "tests/workflow_yaml_test.rs"]
mod tests;
