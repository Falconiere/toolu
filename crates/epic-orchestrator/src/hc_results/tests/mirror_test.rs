#[test]
fn mirror_refuses_another_branch_even_at_the_same_commit() {
  let l = crate::finish_rig::layout();
  let body = crate::finish_rig::read(&l.record);
  let record = body.as_object().expect("record");
  let env = toolu_runtime::env::Env::process();
  assert!(super::verify_mirror(record, &env).is_ok());
  crate::support::git(&l.wt, &["checkout", "-q", "-b", "other-branch"]);
  let error = super::verify_mirror(record, &env).expect_err("foreign branch");
  assert!(error.contains("branch identity"));
}
