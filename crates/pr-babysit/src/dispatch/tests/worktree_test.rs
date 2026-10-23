use super::*;
use crate::dispatch::sandbox::{Sandbox, git, sandbox};

const SLOT_BRANCH: &str = "pr-babysit/falconiere-toolu-165";

fn native(sb: &Sandbox) -> Result<Value> {
  sb.herdr(r#"{"result":{}}"#, "true");
  Dispatcher::new(&sb.options(false))?.worktree(&sb.root, "feat/fix")
}

#[test]
fn a_herdr_worktree_is_created_then_fast_forwarded_on_reuse() {
  let sb = sandbox();
  let created = native(&sb).unwrap();
  let path = created["path"].as_str().unwrap().to_owned();
  assert_eq!(created["branch"], SLOT_BRANCH);
  assert_eq!(created["workspaceId"], "w1");
  assert_eq!(sb.state()["herdrWorktree"], created);
  std::fs::write(sb.repo.join("more.ts"), "export {};\n").unwrap();
  git(&sb.env, &sb.repo, &["add", "more.ts"]);
  git(
    &sb.env,
    &sb.repo,
    &["commit", "--quiet", "-m", "feat: more"],
  );
  git(
    &sb.env,
    &sb.repo,
    &["push", "--quiet", "origin", "feat/fix"],
  );
  assert_eq!(native(&sb).unwrap(), created);
  let head = git(&sb.env, Path::new(&path), &["rev-parse", "HEAD"]);
  assert_eq!(head, git(&sb.env, &sb.repo, &["rev-parse", "HEAD"]));
}

#[test]
fn a_rewritten_pr_branch_or_a_dirty_worktree_refuses_reuse() {
  let sb = sandbox();
  let path = native(&sb).unwrap()["path"].as_str().unwrap().to_owned();
  std::fs::create_dir(Path::new(&path).join(".cursor")).unwrap();
  std::fs::write(Path::new(&path).join(".cursor/package.json"), "{}").unwrap();
  std::fs::write(Path::new(&path).join("extra.ts"), "export {};\n").unwrap();
  let dirty = native(&sb).unwrap_err();
  assert_eq!(dirty.code, "worktree_dirty");
  assert_eq!(dirty.extra["changes"], serde_json::json!(["?? extra.ts"]));
  std::fs::remove_file(Path::new(&path).join("extra.ts")).unwrap();
  for step in [
    &["checkout", "--quiet", "-b", "rewrite", "main"][..],
    &["commit", "--quiet", "--allow-empty", "-m", "rewritten"],
    &["push", "--quiet", "--force", "origin", "rewrite:feat/fix"],
  ] {
    git(&sb.env, &sb.repo, step);
  }
  let stale = native(&sb).unwrap_err();
  assert_eq!(stale.code, "stale_branch");
  assert!(
    stale.message.contains("run dispatch-fix cleanup"),
    "{}",
    stale.message
  );
}

#[test]
fn a_local_slot_branch_with_unpushed_commits_is_never_dropped() {
  let sb = sandbox();
  git(
    &sb.env,
    &sb.repo,
    &["checkout", "--quiet", "-b", SLOT_BRANCH],
  );
  git(
    &sb.env,
    &sb.repo,
    &["commit", "--quiet", "--allow-empty", "-m", "local work"],
  );
  git(&sb.env, &sb.repo, &["checkout", "--quiet", "feat/fix"]);
  let stale = native(&sb).unwrap_err();
  assert_eq!(stale.code, "stale_branch");
  assert_eq!(stale.extra["branch"], SLOT_BRANCH);
  git(
    &sb.env,
    &sb.repo,
    &["branch", "--quiet", "-f", SLOT_BRANCH, "feat/fix"],
  );
  assert_eq!(native(&sb).unwrap()["branch"], SLOT_BRANCH);
}

#[test]
fn muse_hook_state_under_toolu_tmp_is_a_session_file_but_a_new_skill_is_work() {
  let sb = sandbox();
  let path = native(&sb).unwrap()["path"].as_str().unwrap().to_owned();
  let telemetry = Path::new(&path).join(".toolu/tmp/muse/telemetry");
  std::fs::create_dir_all(&telemetry).unwrap();
  std::fs::write(telemetry.join("hook.json"), "{}").unwrap();
  assert!(native(&sb).is_ok());
  let skill = Path::new(&path).join(".toolu/skills/fix");
  std::fs::create_dir_all(&skill).unwrap();
  std::fs::write(skill.join("SKILL.md"), "# fix\n").unwrap();
  let dirty = native(&sb).unwrap_err();
  assert_eq!(dirty.code, "worktree_dirty");
  assert_eq!(
    dirty.extra["changes"],
    serde_json::json!(["?? .toolu/skills/fix/SKILL.md"])
  );
}
