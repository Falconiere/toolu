//! The statusline case file, typed accessors over its JSON, and the per-kind replay loop.

use serde_json::Value;

/// Errors from a real subprocess or fixture file in these integration tests.
pub(crate) type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// The binary built for this integration test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

const FIXTURE: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../fixtures/statusline/cases.json"
);

/// A test error with this message.
pub(crate) fn bad(message: impl Into<String>) -> Box<dyn std::error::Error> {
  std::io::Error::other(message.into()).into()
}

/// Every case of `fixtures/statusline/cases.json`, in file order.
pub(crate) fn all_cases() -> TestResult<Vec<Value>> {
  let doc: Value = serde_json::from_str(&std::fs::read_to_string(FIXTURE)?)?;
  match doc.get("cases") {
    Some(Value::Array(cases)) => Ok(cases.clone()),
    Some(_) | None => Err(bad("cases.json has no cases array")),
  }
}

/// The string field `key` of `value`.
pub(crate) fn str_of<'a>(value: &'a Value, key: &str) -> TestResult<&'a str> {
  value
    .get(key)
    .and_then(Value::as_str)
    .ok_or_else(|| bad(format!("missing string field {key}")))
}

/// The array field `key` of `value`.
pub(crate) fn arr_of<'a>(value: &'a Value, key: &str) -> TestResult<&'a [Value]> {
  value
    .get(key)
    .and_then(Value::as_array)
    .map(Vec::as_slice)
    .ok_or_else(|| bad(format!("missing array field {key}")))
}

/// The field `key`, which the case must carry.
pub(crate) fn field<'a>(value: &'a Value, key: &str) -> TestResult<&'a Value> {
  value
    .get(key)
    .ok_or_else(|| bad(format!("missing field {key}")))
}

/// The integer field `key` of `value`.
pub(crate) fn int_of(value: &Value, key: &str) -> TestResult<i64> {
  value
    .get(key)
    .and_then(Value::as_i64)
    .ok_or_else(|| bad(format!("missing integer field {key}")))
}

/// Replay every case of `kind` through `each`; the error names every case that mismatched.
pub(crate) fn replay(kind: &str, each: impl Fn(&Value) -> TestResult<()>) -> TestResult<usize> {
  let mut failures = Vec::new();
  let mut count = 0;
  for case in all_cases()? {
    if str_of(&case, "kind")? != kind {
      continue;
    }
    count += 1;
    if let Err(error) = each(&case) {
      failures.push(format!("case {:?}: {error}", str_of(&case, "name")?));
    }
  }
  if failures.is_empty() {
    Ok(count)
  } else {
    Err(bad(format!(
      "{} of {count} {kind} cases mismatched:\n{}",
      failures.len(),
      failures.join("\n")
    )))
  }
}
