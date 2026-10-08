use super::*;

#[test]
fn attestation_must_name_the_exact_diff() {
  let dir = tempfile::tempdir().expect("tempdir");
  let file = dir.path().join("docs.json");
  std::fs::write(&file, "{\"diff_sha\":\"new\",\"decision\":\"not-needed\"}").expect("file");
  assert_eq!(attested(&file, "new").as_deref(), Some("not-needed"));
  assert_eq!(attested(&file, "old"), None);
}
