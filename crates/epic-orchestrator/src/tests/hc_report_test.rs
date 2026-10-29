use super::validate;

#[test]
fn report_requires_complete_bound_identities() {
  let sha = "a".repeat(40);
  assert!(validate("run-1", &sha, &sha).is_ok());
  assert!(validate("run/other", &sha, &sha).is_err());
  assert!(validate("run-1", "main", &sha).is_err());
  assert!(validate("run-1", &sha, "HEAD").is_err());
}
