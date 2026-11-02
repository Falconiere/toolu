use super::*;

#[test]
fn corrupt_ledger_is_not_replaced_or_treated_as_empty() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("relay.json");
  std::fs::write(&path, "interrupted write").unwrap();
  let store = Store::new(dir.path());
  assert!(store.list().is_err());
  assert!(store.cancel("missing").is_err());
  assert_eq!(std::fs::read_to_string(path).unwrap(), "interrupted write");
}
