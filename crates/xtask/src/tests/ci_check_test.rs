use std::path::PathBuf;

use crate::Verdict;
use crate::options::Options;

use super::{check, run};

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
fn this_repository_is_consistent() {
  assert_eq!(run(&options(repo())).unwrap(), Verdict::Clean);
}

#[test]
fn a_missing_workflow_file_is_a_finding() {
  let config = crate::ci_model::load_repo(&repo()).unwrap();
  let problems = check(&config, &[], &[]);
  assert!(
    problems
      .iter()
      .any(|line| line.contains("workflow tests.yml does not exist")),
    "{problems:?}"
  );
}
