use std::fs;

use toolu_runtime::json::ordered::Ordered;

use super::prior_round;

#[test]
fn an_old_state_increments_only_for_the_same_diff() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("feature.json");
  let old = include_str!("../../../cli/tests/fixtures/review_legacy.json");
  fs::write(&path, old).unwrap();
  let sha = Ordered::parse(old)
    .unwrap()
    .get("diff_sha")
    .cloned()
    .unwrap();
  let Ordered::String(sha) = sha else {
    panic!("fixture diff hash must be a string");
  };
  assert_eq!(prior_round(&path, &sha), 1);
  assert_eq!(prior_round(&path, "changed"), 0);
  fs::write(&path, "not json").unwrap();
  assert_eq!(prior_round(&path, &sha), 0);
}
