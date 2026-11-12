//! Shared real Git worktree and occupied job lease fixtures for job tests.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use toolu_engine::resources::binding::bind_worktree;
use toolu_engine::resources::leases::{LeaseKind, LeaseRequest, acquire_lease};
use toolu_runtime::json::ordered::Ordered;

/// A bound git worktree and its resource root under `tmp`.
pub(crate) fn bound(tmp: &Path, key: &str) -> (PathBuf, PathBuf) {
  let work = tmp.join("work");
  std::fs::create_dir_all(&work).expect("work");
  let init = Command::new("git")
    .args(["init", "-q"])
    .current_dir(&work)
    .status()
    .expect("git init");
  assert!(init.success());
  let root = tmp.join("resources");
  std::fs::create_dir_all(&root).expect("root");
  bind_worktree(&root, &work, key, &work.display().to_string()).expect("bind");
  (work, root)
}

/// Occupy the only job slot; return the lease token.
pub(crate) fn hold(root: &Path) -> String {
  let occupied = LeaseRequest {
    kind: LeaseKind::Job,
    key: "occupied".to_owned(),
    state_dir: "other".to_owned(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(root, &occupied, SystemTime::now()).expect("lease");
  let Some(Ordered::String(token)) = lease.get("token") else {
    panic!("lease without a token: {lease:?}");
  };
  token.clone()
}
