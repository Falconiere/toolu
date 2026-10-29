//! Preserve a settled remote attempt before beginning a new fixer round.

use super::dispatcher::Dispatcher;
use super::herdr::argv;
use crate::error::{Failure, Result};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

impl Dispatcher<'_> {
  pub(super) fn new_remote_attempt(&self) -> bool {
    matches!(
      self
        .last_remote_fixer()
        .and_then(|fixer| fixer.get("status"))
        .and_then(Value::as_str),
      Some("done" | "failed")
    ) && self.remote().and_then(|remote| remote.get("setupPending")) != Some(&Value::Bool(true))
  }

  pub(super) fn require_prior_stopped(&self) -> Result<()> {
    let groups = self
      .last_remote_fixer()
      .and_then(|fixer| fixer.get("groups"))
      .and_then(Value::as_array)
      .ok_or_else(|| Failure::new("state_malformed", "completed fixer has no groups"))?;
    for group in groups {
      if !matches!(
        group.get("status").and_then(Value::as_str),
        Some("done" | "failed" | "pending")
      ) {
        return Err(Failure::new(
          "fixer_running",
          "prior remote fixer is still active",
        ));
      }
      self.ensure_terminal_stopped(group)?;
    }
    Ok(())
  }

  fn last_remote_fixer(&self) -> Option<&Value> {
    self.fixer().or_else(|| {
      self
        .remote()?
        .get("lastFixer")
        .filter(|fixer| fixer.is_object())
    })
  }

  fn ensure_terminal_stopped(&self, group: &Value) -> Result<()> {
    let has_terminal = ["hcName", "hcAddress"]
      .iter()
      .any(|key| group.get(key).and_then(Value::as_str).is_some());
    if !has_terminal {
      return Ok(());
    }
    let terminal = self.hc_terminal(group)?;
    if terminal.get("state").and_then(Value::as_str) != Some("finished") {
      return Err(Failure::new(
        "fixer_running",
        "prior remote terminal is still live",
      ));
    }
    Ok(())
  }

  pub(super) fn record_remote(&mut self, machine: &str, repo: &str, base: &str) -> Result<()> {
    if self
      .state
      .get("remoteHistory")
      .is_some_and(|value| !value.is_array())
    {
      return Err(Failure::new(
        "state_malformed",
        "remote history is not an array",
      ));
    }
    let path = self.worktree_field("path").unwrap_or_default().to_owned();
    let head = self.git(
      &argv(&["git", "-C", &path, "rev-parse", "HEAD"]),
      "git_error",
      "cannot read fixer head".into(),
    )?;
    let tick = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map_or(0, |d| d.as_nanos());
    let hash = crate::sha1::hex(format!("{}:{tick}", self.file.display()).as_bytes());
    let short: String = hash.chars().take(24).collect();
    let state = json!({"machine": machine, "repo": repo, "run": format!("pb-{short}"), "expectedBase": head, "baseBranch": base, "setupPending": true});
    self.save(|root| {
      let old = root.pointer("/herdrWorktree/remote").cloned();
      let fixer = root
        .get("fixer")
        .filter(|fixer| fixer.is_object())
        .cloned()
        .or_else(|| root.pointer("/herdrWorktree/remote/lastFixer").cloned());
      if let Some(tree) = root.get_mut("herdrWorktree").and_then(Value::as_object_mut) {
        tree.insert("remote".into(), state);
      }
      let history = root
        .as_object_mut()
        .map(|root| root.entry("remoteHistory").or_insert_with(|| json!([])));
      if let (Some(old), Some(history)) = (old, history.and_then(Value::as_array_mut)) {
        history.push(json!({"remote": old, "fixer": fixer}));
      }
    })
  }
}

#[cfg(test)]
#[path = "tests/remote_round_test.rs"]
mod tests;
