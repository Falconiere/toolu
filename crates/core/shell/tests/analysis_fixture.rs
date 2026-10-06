//! The shared analysis fixture (#416 AC-2): for every input of
//! `fixtures/shell/unbash-baseline.json`, the Rust analysis projects to the
//! TypeScript `expect` of `fixtures/shell/analysis.json`, or to `rust.expect`
//! where `rust.reason` records an intended difference
//! (`packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts`).

#[path = "helpers/fixture.rs"]
mod fixture;
#[path = "helpers/project.rs"]
mod project;

use serde_json::Value;

const FIXTURE: &str = "fixtures/shell/analysis.json";

/// The fields of `actual` that differ from `expected`, for a readable failure.
fn differing(expected: &Value, actual: &Value) -> Vec<String> {
  let keys = expected
    .as_object()
    .map(|map| map.keys().cloned().collect())
    .unwrap_or_else(Vec::new);
  keys
    .into_iter()
    .filter(|key| expected[key] != actual[key])
    .map(|key| {
      format!(
        "  {key}:\n    want {}\n    got  {}",
        expected[&key], actual[&key]
      )
    })
    .collect()
}

#[test]
fn the_fixture_has_every_baseline_input_in_order() {
  let inputs: Vec<String> = fixture::cases(FIXTURE)
    .iter()
    .map(|case| fixture::text(case, "input").to_owned())
    .collect();
  let baseline: Vec<String> = fixture::cases("fixtures/shell/unbash-baseline.json")
    .iter()
    .map(|case| fixture::text(case, "input").to_owned())
    .collect();
  assert_eq!(inputs.len(), 203);
  assert_eq!(inputs, baseline);
}

#[test]
fn rust_reproduces_every_projected_analysis() {
  let mut failures = Vec::new();
  for case in fixture::cases(FIXTURE) {
    let input = fixture::text(&case, "input");
    let expected = case
      .get("rust")
      .map_or(&case["expect"], |rust| &rust["expect"]);
    let actual = project::project(&toolu_shell::analyze(input));
    if &actual != expected {
      failures.push(format!(
        "{input:?}\n{}",
        differing(expected, &actual).join("\n")
      ));
    }
  }
  assert!(
    failures.is_empty(),
    "{} differ:\n{}",
    failures.len(),
    failures.join("\n\n")
  );
}

#[test]
fn every_intended_difference_has_a_reason() {
  for case in fixture::cases(FIXTURE) {
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
