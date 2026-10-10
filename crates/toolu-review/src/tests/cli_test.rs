use super::command;

#[test]
fn writer_requires_a_findings_count() {
  assert!(
    command()
      .try_get_matches_from(["review", "write-state"])
      .is_err()
  );
  assert!(
    command()
      .try_get_matches_from(["review", "write-state", "--findings-count", "0"])
      .is_ok()
  );
}
