//! Verify and clean up a remote fixer worktree.

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command};
use super::remote::{fail, field, transfer};
use crate::error::{Failure, Result};
use serde_json::Value;
use std::time::Duration;
use toolu_mux::hc::remote::Transfer;

pub(super) fn verify_remote_result(
  transport: &Transfer<'_>,
  remote: &Value,
  data: &Value,
  commit: &str,
) -> Result<()> {
  let head = clean_head(transport, remote)?;
  if head != commit {
    return Err(Failure::new(
      "remote_error",
      "remote HEAD differs from reported commit",
    ));
  }
  transport
    .git(&["merge-base", "--is-ancestor", field(data, "base")?, commit])
    .map_err(|error| fail(&error))?;
  Ok(())
}

fn clean_head(transport: &Transfer<'_>, remote: &Value) -> Result<String> {
  let path = field(remote, "path")?;
  let status = transport
    .git(&["-C", path, "status", "--porcelain", "--untracked-files=all"])
    .map_err(|error| fail(&error))?;
  if !status.is_empty() {
    return Err(Failure::new(
      "worktree_dirty",
      "remote fixer worktree has uncommitted changes",
    ));
  }
  transport
    .git(&["-C", path, "rev-parse", "HEAD"])
    .map_err(|error| fail(&error))
}

impl Dispatcher<'_> {
  pub(super) fn remove_remote(&self) -> Result<()> {
    if self.fixer_field("status").and_then(Value::as_str) != Some("done") {
      return Err(Failure::new(
        "remote_preserved",
        "remote fixer has not completed; inspect its worktree before cleanup",
      ));
    }
    if self.dry {
      return Err(Failure::usage("remote cleanup cannot be dry-run"));
    }
    let remote = self
      .remote()
      .ok_or_else(|| Failure::new("state_malformed", "missing remote"))?;
    let path = field(remote, "path")?;
    let transport = transfer(self.env, remote, field(remote, "run")?)?;
    let head = clean_head(&transport, remote)?;
    let local = self.worktree_field("path").unwrap_or_default();
    let local_head = command(
      self.env,
      &argv(&["git", "-C", local, "rev-parse", "HEAD"]),
      Duration::from_secs(30),
    );
    if local_head.status != 0 || local_head.output.trim() != head {
      return Err(Failure::new(
        "remote_untransferred",
        "remote commits are not present in the local fixer worktree",
      ));
    }
    transport
      .git(&["worktree", "remove", path])
      .map_err(|error| fail(&error))?;
    Ok(())
  }
}

#[cfg(test)]
#[path = "tests/remote_finish_test.rs"]
mod tests;
