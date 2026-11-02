use super::super::tests::{join, setup};

#[test]
fn damaged_and_future_ledgers_are_never_overwritten() {
  for (content, code) in [
    ("{broken", "storage"),
    (
      r#"{"version":2,"groups":{},"sessions":{},"messages":{}}"#,
      "invalid",
    ),
  ] {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    let path = dir.path().join("peers/relay/foundation/ledger.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    let result = join(dir.path(), "alice");
    assert_eq!(result["error"]["code"], code);
    assert_eq!(std::fs::read_to_string(path).unwrap(), content);
  }
}

#[test]
fn inaccessible_storage_returns_retryable_failure() {
  let dir = tempfile::tempdir().unwrap();
  setup(dir.path());
  std::fs::write(dir.path().join("peers"), "not a directory").unwrap();
  let result = join(dir.path(), "alice");
  assert_eq!(result["error"]["code"], "storage");
  assert_eq!(result["error"]["retryable"], true);
}
