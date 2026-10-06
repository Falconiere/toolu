//! The shared analysis fixture (#416 AC-2): for every input of
//! `fixtures/shell/unbash-baseline.json`, the Rust analysis projects to the
//! TypeScript `expect` of `fixtures/shell/analysis.json`, or to `rust.expect`
//! where `rust.reason` records an intended difference
//! (`packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts`).

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/project.rs"]
mod project;

use cases::{Res, field};
use serde_json::Value;

const FIXTURE: &str = "fixtures/shell/analysis.json";

/// The fields of `actual` that differ from `expected`, for a readable failure.
fn differing(expected: &Value, actual: &Value) -> Vec<String> {
  let Some(fields) = expected.as_object() else {
    return vec![format!("  want {expected}\n  got  {actual}")];
  };
  let mismatched = fields
    .iter()
    .filter(|(key, want)| actual.get(key.as_str()) != Some(*want));
  let shown = mismatched.map(|(key, want)| {
    let got = actual.get(key.as_str()).unwrap_or(&Value::Null);
    format!("  {key}:\n    want {want}\n    got  {got}")
  });
  shown.collect()
}

/// The inputs of the fixture `rel`, in order.
fn inputs(rel: &str) -> Res<Vec<String>> {
  let found = cases::cases(rel)?;
  found
    .iter()
    .map(|case| field(case, "input").map(str::to_owned))
    .collect()
}

/// The answer a case expects from Rust: its `rust.expect`, else its `expect`.
fn expected(case: &Value) -> Res<&Value> {
  let rust = case.get("rust").and_then(|rust| rust.get("expect"));
  rust
    .or_else(|| case.get("expect"))
    .ok_or_else(|| format!("no expect in {case}"))
}

/// Every case whose Rust projection differs, with the differing fields.
fn failures() -> Res<Vec<String>> {
  let mut failures = Vec::new();
  for case in cases::cases(FIXTURE)? {
    let input = field(&case, "input")?;
    let actual = project::project(&toolu_shell::analyze(input));
    let want = expected(&case)?;
    if &actual != want {
      failures.push(format!(
        "{input:?}\n{}",
        differing(want, &actual).join("\n")
      ));
    }
  }
  Ok(failures)
}

#[test]
fn the_fixture_has_every_baseline_input_in_order() {
  let fixture = inputs(FIXTURE).unwrap();
  assert_eq!(fixture.len(), 203);
  assert_eq!(
    fixture,
    inputs("fixtures/shell/unbash-baseline.json").unwrap()
  );
}

#[test]
fn rust_reproduces_every_projected_analysis() {
  let failures = failures().unwrap();
  assert!(
    failures.is_empty(),
    "{} differ:\n{}",
    failures.len(),
    failures.join("\n\n")
  );
}

#[test]
fn every_intended_difference_has_a_reason() {
  for case in cases::cases(FIXTURE).unwrap() {
    if let Some(rust) = case.get("rust") {
      assert!(
        rust["reason"]
          .as_str()
          .is_some_and(|reason| !reason.is_empty()),
        "{case}"
      );
      assert_ne!(rust["expect"], case["expect"], "{case}");
    }
  }
}
