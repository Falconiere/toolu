use super::read_workflows;

#[test]
fn a_workflow_without_jobs_is_rejected() {
  let root = std::env::temp_dir().join(format!("toolu-yaml-{}", std::process::id()));
  let dir = root.join(".github/workflows");
  std::fs::create_dir_all(&dir).expect("dir");
  std::fs::write(dir.join("tests.yml"), "name: tests\n").expect("write");
  let error = read_workflows(&dir).expect_err("jobs");
  assert!(error.contains("not a workflow"), "{error}");
  std::fs::remove_dir_all(&root).expect("cleanup");
}
