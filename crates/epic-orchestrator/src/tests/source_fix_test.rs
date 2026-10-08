use super::fixture;

#[test]
fn fixture_builds_a_running_issue() {
  let fix = fixture(true).expect("fixture");
  assert!(fix.env.get("HERDR_SOCKET_PATH").is_some());
  let rows = fix.script.agents.as_array().expect("agents");
  assert_eq!(rows.len(), 1);
}
