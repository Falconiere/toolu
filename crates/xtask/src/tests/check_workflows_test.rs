use super::run;
use crate::Verdict;
use crate::options::Options;

#[test]
fn committed_workflows_pass() {
  let options = Options::parse(&[]).unwrap();
  assert_eq!(run(&options).unwrap(), Verdict::Clean);
}

#[test]
fn absent_workflows_fail_closed() {
  let temp = tempfile::TempDir::new().unwrap();
  let options = Options::parse(&[
    "--root".to_owned(),
    temp.path().to_string_lossy().to_string(),
  ])
  .unwrap();
  assert_eq!(run(&options).unwrap(), Verdict::Findings);
}
