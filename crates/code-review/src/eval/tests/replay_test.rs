use super::Store;
use crate::review::Stage;

#[test]
fn the_key_is_the_stage_and_the_sha256_of_the_exact_request() {
  // SHA-256("abc"), from FIPS 180-2.
  assert_eq!(
    Store::key(Stage::Review, "abc"),
    "review-ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  );
  assert_ne!(
    Store::key(Stage::Review, "abc"),
    Store::key(Stage::Judge, "abc")
  );
  assert_ne!(
    Store::key(Stage::Review, "abc"),
    Store::key(Stage::Review, "abc ")
  );
}

#[test]
fn a_written_response_reads_back_and_a_different_request_misses() {
  let dir = tempfile::tempdir().expect("fs");
  let store = Store::new(&dir.path().join("replay"));
  assert_eq!(store.read(Stage::Review, "req"), Ok(None));
  store.write(Stage::Review, "req", "{\"a\":1}").expect("fs");
  assert_eq!(
    store.read(Stage::Review, "req"),
    Ok(Some("{\"a\":1}".to_owned()))
  );
  assert_eq!(store.read(Stage::Review, "other"), Ok(None));
  assert_eq!(store.read(Stage::Recheck, "req"), Ok(None));
  let file = dir
    .path()
    .join("replay")
    .join(format!("{}.json", Store::key(Stage::Review, "req")));
  assert!(file.is_file());
}

#[test]
fn a_recorded_response_drops_the_provider_id_and_keeps_everything_else() {
  let dir = tempfile::tempdir().expect("fs");
  let store = Store::new(dir.path());
  let body = r#"{"model":"m","answers":{"f0":{"choice":"supported"}},"id":"gen-dec-1-UUtNQdfWtNEROCSSnzrj"}"#;
  store.write(Stage::Judge, "req", body).expect("fs");
  let read = store
    .read(Stage::Judge, "req")
    .expect("fs")
    .expect("recorded");
  let value: serde_json::Value = serde_json::from_str(&read).expect("json");
  assert_eq!(
    value,
    serde_json::json!({"model": "m", "answers": {"f0": {"choice": "supported"}}})
  );
  // Text that is not an object with an id is stored byte for byte.
  for kept in ["{\"a\":1}", "[1,2]", "not json"] {
    store.write(Stage::Review, kept, kept).expect("fs");
    assert_eq!(store.read(Stage::Review, kept), Ok(Some(kept.to_owned())));
  }
}
