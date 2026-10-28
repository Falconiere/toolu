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
