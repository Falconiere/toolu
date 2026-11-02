use super::{Fixture, Target, Transfer};

#[test]
fn private_file_upload_retries_identical_bytes_and_rejects_conflicts() {
  let fixture = Fixture::new().unwrap();
  let repo = tempfile::tempdir_in(fixture.dir.path()).unwrap();
  let root = repo.path().to_str().unwrap();
  fixture.git(root, &["init", "-q"]).unwrap();
  let transfer = Transfer {
    target: Target::new(Some("loop"), Some(root)).unwrap().unwrap(),
    env: &fixture.env,
    run: "private-file",
  };
  let path = transfer.upload_file("toolu", b"binary\0data").unwrap();
  assert_eq!(
    transfer.upload_file("toolu", b"binary\0data").unwrap(),
    path
  );
  assert!(transfer.upload_file("toolu", b"different").is_err());
  assert_eq!(std::fs::read(path).unwrap(), b"binary\0data");
}
