use super::*;

#[test]
fn remote_state_requires_nonempty_fields() {
  assert!(field(&json!({"run": "pb-a"}), "run").is_ok());
  assert!(field(&json!({"run": ""}), "run").is_err());
  assert!(field(&json!({}), "run").is_err());
}

#[test]
fn report_requires_the_exact_run_base_and_full_lowercase_commit() {
  let base = "a".repeat(40);
  let commit = "b".repeat(40);
  let expected = json!({"run": "pb-test-g1", "base": base});
  let good = json!({"version": 1, "status": "done", "runId": "pb-test-g1", "expectedBase": base, "commit": commit});
  assert_eq!(validate_report(&good, &expected).unwrap(), commit);
  for (key, value) in [
    ("runId", json!("other")),
    ("expectedBase", json!("bad")),
    ("commit", json!("ABC")),
    ("status", json!("failed")),
  ] {
    let mut bad = good.clone();
    bad[key] = value;
    assert!(validate_report(&bad, &expected).is_err(), "{key}");
  }
  assert!(validate_report(&good, &json!({})).is_err());
}

#[test]
fn committed_transfer_recovery_keeps_the_same_git_snapshot() {
  let sb = crate::dispatch::sandbox::sandbox();
  let head = crate::dispatch::sandbox::git(&sb.env, &sb.repo, &["rev-parse", "HEAD"]);
  let repo = sb.repo.to_str().unwrap();
  let target = sb.origin.to_str().unwrap();
  let first = remote::export_recover(&sb.env, repo, "pb-transfer-test", &head).unwrap();
  let again = remote::export_recover(&sb.env, repo, "pb-transfer-test", &head).unwrap();
  assert_eq!(first, again);
  remote::import_recover(&sb.env, target, "pb-transfer-test", &head, &first).unwrap();
  remote::import_recover(&sb.env, target, "pb-transfer-test", &head, &first).unwrap();
  let other = crate::dispatch::sandbox::git(&sb.env, &sb.repo, &["rev-parse", "HEAD^"]);
  assert!(remote::export_recover(&sb.env, repo, "pb-transfer-test", &other).is_err());
  assert!(remote::import_recover(&sb.env, target, "pb-transfer-test", &other, &first).is_err());
}
