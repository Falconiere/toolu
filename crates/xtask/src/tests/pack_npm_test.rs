use super::packed_files;

#[test]
fn a_dry_run_lists_package_json() {
  let dir = tempfile::tempdir().expect("temp");
  let manifest = r#"{"name":"pack-fixture","version":"0.0.0"}"#;
  std::fs::write(dir.path().join("package.json"), manifest).expect("manifest");
  let files = packed_files(dir.path()).expect("pack");
  assert!(files.iter().any(|file| file.path == "package.json"));
}
