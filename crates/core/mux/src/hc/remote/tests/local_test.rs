use super::local_git;

#[test]
fn rejects_missing_repository() {
  assert!(
    local_git(
      &toolu_runtime::env::Env::process(),
      "/no-such-toolu-repository",
      &["rev-parse", "HEAD"]
    )
    .is_err()
  );
}
