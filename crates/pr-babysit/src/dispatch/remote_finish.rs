//! Verify and clean up a remote fixer worktree.

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command};
use super::remote::{fail, field, transfer};
use crate::error::{Failure, Result};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use toolu_mux::hc::remote::Transfer;

/// Require a clean committed result descending from the dispatched base.
pub(super) fn verify_remote_result(
  transport: &Transfer<'_>,
  remote: &Value,
  data: &Value,
  commit: &str,
) -> Result<()> {
  ensure_clean(transport, remote)?;
  verify_reported_commit(transport, remote, data, commit)
}

/// Require the reported commit to match remote HEAD and descend from the dispatched base.
pub(super) fn verify_reported_commit(
  transport: &Transfer<'_>,
  remote: &Value,
  data: &Value,
  commit: &str,
) -> Result<()> {
  if remote_head(transport, remote)? != commit {
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

fn remote_head(transport: &Transfer<'_>, remote: &Value) -> Result<String> {
  let path = field(remote, "path")?;
  transport
    .git(&["-C", path, "rev-parse", "HEAD"])
    .map_err(|error| fail(&error))
}

fn ensure_clean(transport: &Transfer<'_>, remote: &Value) -> Result<()> {
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
  Ok(())
}

fn clean_head(transport: &Transfer<'_>, remote: &Value) -> Result<String> {
  ensure_clean(transport, remote)?;
  remote_head(transport, remote)
}

impl Dispatcher<'_> {
  /// Remove verified remote work or reconcile its recorded removal.
  pub(super) fn remove_remote(&mut self) -> Result<()> {
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
      .cloned()
      .ok_or_else(|| Failure::new("state_malformed", "missing remote"))?;
    let path = field(&remote, "path")?;
    let transport = transfer(self.env, &remote, field(&remote, "run")?)?;
    if self.removed_remote(&remote, &transport)? {
      return Ok(());
    }
    let head = self.cleanup_head()?;
    let intent = json!({"remote": remote, "head": head});
    if registered(&transport, path)? {
      if clean_head(&transport, &remote)? != head {
        return Err(Failure::new(
          "remote_untransferred",
          "remote commits are not present in the local fixer worktree",
        ));
      }
      self.patch_fixer(&json!({"remoteRemoval": intent}))?;
      transport
        .git(&["worktree", "remove", path])
        .map_err(|error| fail(&error))?;
    } else if self.fixer_field("remoteRemoval") != Some(&intent) {
      return Err(Failure::new(
        "remote_preserved",
        "missing remote worktree has no matching removal intent",
      ));
    }
    absent(&transport, path)?;
    self.save(|state| {
      if let Some(Value::Object(worktree)) = state.get_mut("herdrWorktree") {
        worktree.insert("remoteRemoval".into(), intent);
      }
    })
  }

  fn removed_remote(&self, remote: &Value, transport: &Transfer<'_>) -> Result<bool> {
    let Some(marker) = self.state.pointer("/herdrWorktree/remoteRemoval") else {
      return Ok(false);
    };
    if marker.get("remote") != Some(remote) || self.fixer_field("remoteRemoval") != Some(marker) {
      return Ok(false);
    }
    let local = self.worktree_field("path").unwrap_or_default();
    if std::path::Path::new(local).exists()
      && marker.get("head").and_then(Value::as_str) != Some(self.cleanup_head()?.as_str())
    {
      return Err(Failure::new(
        "remote_untransferred",
        "local fixer result changed after cleanup",
      ));
    }
    absent(transport, field(remote, "path")?)?;
    Ok(true)
  }

  fn cleanup_head(&self) -> Result<String> {
    let local = self.worktree_field("path").unwrap_or_default();
    let head = command(
      self.env,
      &argv(&["git", "-C", local, "rev-parse", "HEAD"]),
      Duration::from_secs(30),
    );
    if head.status != 0 {
      return Err(Failure::new(
        "remote_untransferred",
        "local fixer result is not readable",
      ));
    }
    Ok(head.output.trim().to_owned())
  }
}

fn registered(transport: &Transfer<'_>, path: &str) -> Result<bool> {
  let listed = transport
    .git(&["worktree", "list", "--porcelain", "-z"])
    .map_err(|error| fail(&error))?;
  if listed
    .split('\0')
    .any(|line| line == format!("worktree {path}"))
  {
    return Ok(true);
  }
  let path = Path::new(path);
  let parent = path
    .parent()
    .and_then(Path::to_str)
    .ok_or_else(|| Failure::new("state_malformed", "invalid remote worktree path"))?;
  let name = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| Failure::new("state_malformed", "invalid remote worktree path"))?;
  let resolved = transport
    .exec(&["realpath", parent], &[])
    .map_err(|error| fail(&error))?;
  let resolved = std::str::from_utf8(&resolved)
    .map_err(|error| Failure::new("remote_error", error.to_string()))?;
  let resolved = Path::new(resolved.trim_end_matches(['\r', '\n']));
  if !resolved.is_absolute() {
    return Err(Failure::new("remote_error", "invalid remote realpath"));
  }
  let expected = format!("worktree {}", resolved.join(name).display());
  Ok(listed.split('\0').any(|line| line == expected))
}

fn absent(transport: &Transfer<'_>, path: &str) -> Result<()> {
  if registered(transport, path)? {
    return Err(Failure::new(
      "remote_preserved",
      "remote worktree is still registered",
    ));
  }
  for kind in ["-e", "-L"] {
    transport
      .exec(&["test", "!", kind, path], &[])
      .map_err(|error| fail(&error))?;
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/remote_finish_test.rs"]
mod tests;
