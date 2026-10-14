//! `dispatch-fix wait`: follow the round group by group until it is done,
//! fails, blocks on a question, or the deadline passes.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command, screen, stop_agent, try_herdr};
use super::launch::Launch;
use super::outcome::fixer_outcome;
use crate::error::{Failure, Result};

/// The wait when the caller names none.
pub(crate) const DEFAULT_TIMEOUT: u64 = 480;

fn text<'v>(value: &'v Value, key: &str) -> &'v str {
  value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn agent_status(value: &Value) -> Option<&str> {
  value.pointer("/agent/agent_status")?.as_str()
}

fn one_line(text: &str, limit: usize) -> String {
  text
    .trim_end()
    .replace('\n', " ")
    .chars()
    .take(limit)
    .collect()
}

impl Dispatcher<'_> {
  fn outcome(&self, group: &Value) -> (&'static str, String) {
    let report = Path::new(text(group, "report"));
    let agent = text(group, "agent");
    let read = screen(self.env, agent);
    let shown = if read.status == 0 {
      read.output.as_str()
    } else {
      ""
    };
    let outcome = fixer_outcome(report, shown);
    stop_agent(self.env, agent);
    let error = if outcome == "no_report" && read.status != 0 {
      format!(
        "could not read the fixer's last screen: {}",
        one_line(&read.output, 160)
      )
    } else {
      String::new()
    };
    (outcome, error)
  }

  fn settle_group(&mut self, seq: u64) -> Result<()> {
    let (outcome, error) = self.outcome(&self.group(seq)?);
    if outcome != "done" {
      return self.settle(seq, outcome, "", &error);
    }
    let worktree = self.worktree_field("path").unwrap_or_default().to_owned();
    let head = command(
      self.env,
      &argv(&["git", "-C", &worktree, "rev-parse", "HEAD"]),
      Duration::from_secs(60),
    );
    if head.status != 0 {
      let error = format!("the fixer reported done, but its worktree is not readable: {worktree}");
      return self.settle(seq, "worktree_lost", "", &error);
    }
    self.settle(seq, "done", head.output.trim(), "")
  }

  /// Ends a group's fixer before it is launched again or cleaned up.
  pub(super) fn stop_fixer(&self, group: &Value) {
    if let Some(agent) = group.get("agent").and_then(Value::as_str) {
      stop_agent(self.env, agent);
    }
  }

  fn unblock(&mut self) -> Result<()> {
    let current = self
      .fixer_field("current")
      .and_then(Value::as_u64)
      .unwrap_or(1);
    let group = self.group(current)?;
    let agent = text(&group, "agent");
    let status = match try_herdr(
      self.env,
      &argv(&["agent", "get", agent]),
      Duration::from_secs(120),
    ) {
      Ok(got) => agent_status(&got).map(str::to_owned),
      Err(error) if error.code == "agent_not_found" => Some("gone".to_owned()),
      Err(error) => {
        let message = format!("herdr agent get {agent}: {}: {}", error.code, error.message);
        return Err(Failure::new("herdr_error", message));
      }
    };
    if status.as_deref() == Some("blocked") {
      return Ok(());
    }
    self.patch_fixer(&json!({"status": "running", "reason": null}))?;
    self.patch_group(current, &json!({"status": "running", "reason": null}))
  }

  fn relaunch(&mut self, seq: u64, group: &Value) -> Result<()> {
    self.stop_fixer(group);
    let items_file = self
      .fixer_field("itemsFile")
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_owned();
    let context = self.fixer_field("context").cloned().unwrap_or(Value::Null);
    let pane = self.worktree_field("paneId").unwrap_or_default().to_owned();
    let unattended = self.fixer_field("unattended") != Some(&Value::Bool(false));
    let launch = Launch {
      seq,
      items_file: Path::new(&items_file),
      context: &context,
      pane: &pane,
      unattended,
    };
    self.launch(&launch, None)?;
    Ok(())
  }
}

impl Dispatcher<'_> {
  /// The herdr agent's status after `herdr agent wait`; `None` on its timeout.
  fn waited(&self, agent: &str, remaining: u64) -> Result<Option<String>> {
    let millis = (remaining * 1000).to_string();
    let wait = argv(&["agent", "wait", agent, "--timeout", &millis]);
    match try_herdr(self.env, &wait, Duration::from_secs(remaining + 60)) {
      Ok(waited) => Ok(Some(agent_status(&waited).unwrap_or("unknown").to_owned())),
      Err(error) if error.code == "timeout" => Ok(None),
      Err(error) if error.code == "agent_not_found" => Ok(Some("exited".to_owned())),
      Err(error) => {
        let message = format!(
          "herdr agent wait {agent}: {}: {}",
          error.code, error.message
        );
        Err(Failure::new("herdr_error", message))
      }
    }
  }

  /// One pass of the wait loop; false when it ends.
  fn step(&mut self, deadline: Instant, fresh: &mut bool) -> Result<bool> {
    let remaining = deadline.saturating_duration_since(Instant::now()).as_secs();
    if remaining == 0 {
      return Ok(false);
    }
    let seq = self
      .fixer_field("current")
      .and_then(Value::as_u64)
      .unwrap_or(1);
    let group = self.group(seq)?;
    let count = self
      .fixer_field("groups")
      .and_then(Value::as_array)
      .map_or(0, Vec::len);
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    if matches!(text(&group, "status"), "pending" | "launching") {
      if !*fresh && remaining < 250 {
        return Ok(false);
      }
      *fresh = false;
      self.relaunch(seq, &group)?;
      return Ok(true);
    }
    *fresh = false;
    let Some(status) = self.waited(text(&group, "agent"), remaining)? else {
      return Ok(false);
    };
    if status == "blocked" {
      let blocked = json!({"status": "blocked", "reason": "agent_blocked"});
      self.patch_fixer(&blocked)?;
      self.patch_group(seq, &blocked)?;
      return Ok(false);
    }
    self.settle_group(seq)?;
    self.next(seq, count)
  }

  /// Follows the round for up to `timeout` seconds; the status document.
  ///
  /// # Errors
  /// herdr failures other than a timeout or a gone agent; state failures.
  pub(crate) fn wait(&mut self, timeout: u64) -> Result<Value> {
    if self.fixer().is_none() {
      return Ok(json!({"version": 1, "status": "none"}));
    }
    let deadline = Instant::now() + Duration::from_secs(timeout);
    if self.fixer_field("status").and_then(Value::as_str) == Some("blocked") {
      self.unblock()?;
    }
    let mut fresh = true;
    while self.fixer_field("status").and_then(Value::as_str) == Some("running") {
      if !self.step(deadline, &mut fresh)? {
        break;
      }
    }
    Ok(self.status())
  }
}

#[cfg(test)]
#[path = "tests/watch_test.rs"]
mod tests;
