#[test]
fn a_result_missing_the_tokenizer_source_is_rejected() {
  let dir = tempfile::tempdir().expect("temp");
  let path = dir.path().join("bad.json");
  std::fs::write(&path, "{\"mechanism\":\"retrieval\"}\n").expect("write");
  let err = super::validate(&path).expect_err("invalid");
  assert!(err.contains("tokenizer.source"), "{err}");
}
