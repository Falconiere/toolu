#[test]
fn backoff_survives_restart_and_reset_clears_deadline() {
  let tmp = tempfile::tempdir().expect("tempdir");
  let path = tmp.path().join("hc-watch.json");
  let mut first = super::Backoff::default();
  first.failed(&path);
  let mut reloaded = super::Backoff::load(&path);
  assert_eq!(reloaded.failures, 1);
  assert_eq!(reloaded.retry_at, first.retry_at);
  assert!(reloaded.remaining() <= std::time::Duration::from_secs(1));
  reloaded.failed(&path);
  assert_eq!(super::Backoff::load(&path).failures, 2);
  super::Backoff::default().save(&path);
  let cleared = super::Backoff::load(&path);
  assert_eq!(cleared.failures, 0);
  assert_eq!(cleared.retry_at, 0);
}
