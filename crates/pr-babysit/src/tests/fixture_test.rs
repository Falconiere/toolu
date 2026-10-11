use super::{json, path, text};

#[test]
fn recorded_fixtures_resolve_from_the_crate() {
  assert!(path("snapshots/toolu-165.json").is_file());
  assert_eq!(json("states/toolu-165-initial.json")["version"], 2);
  assert!(text("verdicts/provider-error.txt").contains("rror"));
  assert!(json("missing.json").is_null());
}
