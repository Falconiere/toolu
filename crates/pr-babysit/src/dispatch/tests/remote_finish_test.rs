use super::*;
use crate::dispatch::sandbox::sandbox;

struct RemoteFixer {
  sb: crate::dispatch::sandbox::Sandbox,
  core: crate::dispatch::sandbox::Core,
  path: String,
}

impl RemoteFixer {
  fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
    let sb = sandbox();
    let core = crate::dispatch::sandbox::Core::new(&sb.env)?;
    toolu_mux::hc::Client::new(&core.env)
      .call(
        &[
          "machine",
          "add",
          "--name",
          "loop",
          "--via",
          "/bin/sh -c",
          "--session",
          "remote",
          "--temp",
        ],
        Duration::from_secs(30),
      )
      .map_err(|e| std::io::Error::other(e.message))?;
    let path = sb.dir.path().join("remote-tree").display().to_string();
    crate::dispatch::sandbox::git(
      &core.env,
      &sb.repo,
      &["worktree", "add", "--detach", &path, "HEAD"],
    );
    let mut options = sb.options(false);
    options.env = &core.env;
    let mut dispatcher = Dispatcher::new(&options)?;
    dispatcher.save(|state| {
      state["herdrWorktree"] = json!({"path":sb.root,"multiplexer":"herdr-app",
        "remote":{"machine":"loop","repo":sb.root,"path":path,"run":"cleanup"}});
      state["fixer"] = json!({"status":"done","groups":[]});
    })?;
    Ok(Self { sb, core, path })
  }

  fn dispatcher(&self) -> Result<Dispatcher<'_>> {
    let mut options = self.sb.options(false);
    options.env = &self.core.env;
    Dispatcher::new(&options)
  }
}

#[test]
fn unfinished_remote_cleanup_preserves_the_recorded_worktree() {
  let sb = sandbox();
  let mut dispatcher = Dispatcher::new(&sb.options(false)).unwrap();
  dispatcher.state["herdrWorktree"] =
    serde_json::json!({"remote": {"machine": "vm", "repo": "/repo", "run": "r"}});
  let error = dispatcher.remove_remote().unwrap_err();
  assert_eq!(error.code, "remote_preserved");
  assert!(dispatcher.remote().is_some());
}

#[test]
fn remote_cleanup_reconciles_its_own_completed_removal_only() {
  let fixture = RemoteFixer::new().unwrap();
  let mut dispatcher = fixture.dispatcher().unwrap();
  dispatcher.remove_remote().unwrap();
  assert!(!std::path::Path::new(&fixture.path).exists());
  fixture.dispatcher().unwrap().remove_remote().unwrap();
  std::fs::write(&fixture.path, "replacement").unwrap();
  assert!(fixture.dispatcher().unwrap().remove_remote().is_err());
  assert_eq!(
    std::fs::read_to_string(&fixture.path).unwrap(),
    "replacement"
  );
}

#[test]
fn a_missing_remote_tree_without_intent_is_not_cleanup_success() {
  let fixture = RemoteFixer::new().unwrap();
  crate::dispatch::sandbox::git(
    &fixture.core.env,
    &fixture.sb.repo,
    &["worktree", "remove", &fixture.path],
  );
  let error = fixture.dispatcher().unwrap().remove_remote().unwrap_err();
  assert_eq!(error.code, "remote_preserved");
  assert!(error.message.contains("matching removal intent"));
}

#[test]
fn remote_cleanup_preserves_later_committed_work() {
  let fixture = RemoteFixer::new().unwrap();
  let tree = std::path::Path::new(&fixture.path);
  std::fs::write(tree.join("later"), "new work").unwrap();
  for args in [&["add", "later"][..], &["commit", "-qm", "later"]] {
    crate::dispatch::sandbox::git(&fixture.core.env, tree, args);
  }
  let error = fixture.dispatcher().unwrap().remove_remote().unwrap_err();
  assert_eq!(error.code, "remote_untransferred");
  assert!(tree.join("later").exists());
}

#[test]
fn cleanup_resumes_after_both_worktrees_are_removed_before_state_is_cleared() {
  let fixture = RemoteFixer::new().unwrap();
  let local = fixture.sb.dir.path().join("local-tree");
  crate::dispatch::sandbox::git(
    &fixture.core.env,
    &fixture.sb.repo,
    &[
      "worktree",
      "add",
      "-b",
      "fixer",
      local.to_str().unwrap(),
      "HEAD",
    ],
  );
  let mut dispatcher = fixture.dispatcher().unwrap();
  dispatcher
    .save(|state| {
      let worktree = &mut state["herdrWorktree"];
      worktree["path"] = json!(local);
      worktree["repoRoot"] = json!(fixture.sb.root);
      worktree["branch"] = json!("fixer");
      worktree["prBranch"] = json!("feat/fix");
    })
    .unwrap();
  dispatcher.remove_remote().unwrap();
  crate::dispatch::sandbox::git(
    &fixture.core.env,
    &fixture.sb.repo,
    &["worktree", "remove", local.to_str().unwrap()],
  );
  let mut resumed = fixture.dispatcher().unwrap();
  assert_eq!(resumed.cleanup().unwrap()["status"], "cleaned");
  assert!(resumed.state["fixer"].is_null());
  assert!(resumed.state["herdrWorktree"].is_null());
}
