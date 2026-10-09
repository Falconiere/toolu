use crate::Verdict;
use crate::task_options::{test_options, test_root};

#[test]
fn this_checkout_passes() {
  assert_eq!(
    super::run(&test_options(test_root())).unwrap(),
    Verdict::Clean
  );
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
