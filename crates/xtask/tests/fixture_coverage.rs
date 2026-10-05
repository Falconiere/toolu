//! Every fixture case under `fixtures/guardrails/rust/` has a test that runs it,
//! so a new case cannot sit unexercised.

use std::fs;
use std::path::Path;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

#[test]
fn every_fixture_case_is_run_by_a_test() {
  // Whitespace is dropped on both sides, so reformatting a test never hides a case.
  let tests: String = [
    "fixtures_guardrails.rs",
    "fixtures_clippy.rs",
    "fixtures_tools.rs",
    "fixtures_gate_change.rs",
  ]
  .iter()
  .map(|file| {
    fs::read_to_string(
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(file),
    )
    .unwrap()
  })
  .collect::<String>()
  .split_whitespace()
  .collect();
  let root = Path::new(REPO).join("fixtures/guardrails/rust");
  let mut missing = Vec::new();
  let mut cases = 0;
  for rule in fs::read_dir(&root).unwrap() {
    let rule = rule.unwrap().file_name().to_string_lossy().into_owned();
    if rule == "base" {
      continue;
    }
    for case in fs::read_dir(root.join(&rule)).unwrap() {
      let case = case.unwrap().file_name().to_string_lossy().into_owned();
      cases += 1;
      if !tests.contains(&format!("fixture::check(\"{rule}\",\"{case}\")")) {
        missing.push(format!("{rule}/{case}"));
      }
    }
  }
  assert!(cases > 90, "{cases} cases");
  assert_eq!(missing, Vec::<String>::new());
}
