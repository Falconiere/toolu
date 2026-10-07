use super::located;

#[test]
fn a_status_path_yields_the_issue_key_and_state_dir() {
  let (key, dir) = located(std::path::Path::new("/epics/one/status/a.json")).expect("path");
  assert_eq!(key, "a");
  assert_eq!(dir, "/epics/one");
}
