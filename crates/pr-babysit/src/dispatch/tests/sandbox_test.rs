use super::*;

#[test]
fn the_sandbox_holds_a_pushed_pr_branch_and_the_captured_slot_state() {
  let sb = sandbox();
  let origin = git(&sb.env, &sb.origin, &["rev-parse", "feat/fix"]);
  assert_eq!(origin, git(&sb.env, &sb.repo, &["rev-parse", "HEAD"]));
  assert_eq!(sb.state()["slot"], "falconiere-toolu-165");
  assert_eq!(
    sb.state()["actions"]["replied"]["2300000001"]["replyId"],
    7234
  );
  let (plan, items) = sb.plan("opencode", None);
  assert!(plan.exists() && items.exists());
  assert_eq!(sb.options(true).state_file, sb.state);
  let start = sb.start(&plan, &items);
  assert_eq!((start.branch, start.base), ("feat/fix", "main"));
  assert_eq!(start.repo_root, sb.repo.display().to_string());
}

#[test]
fn a_stand_in_opencode_is_an_executable_on_the_sandbox_path() {
  let sb = sandbox();
  sb.opencode(FIX, "done");
  assert!(crate::route::on_path(&sb.env, "opencode"));
  assert!(!crate::route::on_path(&sb.env, "herdr"));
}
