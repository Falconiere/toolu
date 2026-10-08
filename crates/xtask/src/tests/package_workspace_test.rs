use std::path::PathBuf;

use crate::Verdict;
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn options(root: PathBuf) -> Options {
  Options {
    root,
    ..Options::default()
  }
}

#[test]
fn this_checkout_passes() {
  assert_eq!(super::run(&options(repo())).unwrap(), Verdict::Clean);
}

#[test]
fn a_missing_package_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  std::fs::write(
    tmp.path().join("package.json"),
    "{ \"name\": \"empty\", \"private\": true }\n",
  )
  .unwrap();
  let problem = super::problem(tmp.path()).unwrap().unwrap();
  assert!(
    problem.contains("missing packages/toolu-core/package.json"),
    "{problem}"
  );
}
