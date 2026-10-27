use super::command;

#[test]
fn commands_require_their_files() {
  assert!(command().try_get_matches_from(["docs", "route"]).is_err());
  assert!(
    command()
      .try_get_matches_from(["docs", "write-state"])
      .is_err()
  );
  assert!(
    command()
      .try_get_matches_from(["docs", "inspect", "--branch", "feature"])
      .is_ok()
  );
}
