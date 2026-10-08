use super::*;

#[test]
fn locate_prefers_an_existing_file_beside_the_hook() {
  let dir = tempfile::tempdir().expect("tempdir");
  let here = dir.path().join("plan.md");
  std::fs::write(&here, "plan").expect("file");
  assert_eq!(locate(dir.path(), Path::new("/elsewhere"), "plan.md"), here);
}
