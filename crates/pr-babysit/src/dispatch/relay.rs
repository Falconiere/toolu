//! Relay-backed fixer groups and their committed results.

use std::time::Duration;

use serde_json::{Value, json};
use toolu_mux::hc::remote;
use toolu_mux::relay::{client, store::Submission};

use super::brief::{Reports, render};
use super::dispatcher::Dispatcher;
use super::herdr::argv;
use super::launch::Launch;
use super::plan::read_object;
use crate::error::{Failure, Result};
use crate::fixer::host::Fixer;

fn error(message: impl Into<String>) -> Failure {
  Failure::new("relay_error", message.into())
}

fn relay_brief(launch: &Launch<'_>, group: &Value) -> Result<String> {
  let items = read_object(
    launch.items_file,
    "plan_invalid",
    "cannot read fixer items".into(),
  )?;
  let entries = items
    .get("items")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  let mut context = launch.context.clone();
  let fields = context
    .as_object_mut()
    .ok_or_else(|| error("invalid fixer context"))?;
  fields.insert("worktree".into(), json!("."));
  let reports = Reports {
    done: "Use the relay success report command provided below.",
    failed: "Use the relay failure report command provided below.",
  };
  Ok(render(&entries, group, &context, &reports))
}

#[cfg(test)]
#[path = "tests/relay_test.rs"]
mod tests;

impl Dispatcher<'_> {
  /// Resolve relay mode without changing the round's saved state.
  pub(super) fn relay_enabled(&self) -> Result<bool> {
    Ok(
      !self.dry
        && toolu_mux::relay::config::load(&toolu_mux::relay::config::root(self.env))
          .map_err(error)?
          .relay
          .is_some(),
    )
  }

  /// Pin this round to relay mode and the requested worker.
  pub(super) fn configure_relay(&mut self, machine: Option<&str>, relay: bool) -> Result<()> {
    self.save(|state| {
      state["relayEnabled"] = json!(relay);
      state["relayMachine"] = json!(machine);
    })
  }

  /// Submit or resume the group's durable request.
  pub(super) fn launch_relay(&mut self, launch: &Launch<'_>, group: &Value) -> Result<String> {
    let seq = launch.seq;
    let request = if let Some(saved) = group.get("relayRequest") {
      serde_json::from_value::<Submission>(saved.clone()).map_err(|e| error(e.to_string()))?
    } else {
      let request = self.relay_request(launch, group)?;
      self.patch_group(
        seq,
        &json!({"relayRequest": request, "status": "launching"}),
      )?;
      request
    };
    let path = self.worktree_field("path").unwrap_or_default();
    let bundle = remote::export_recover(self.env, path, &request.key, &request.base)
      .map_err(|e| error(e.message))?;
    client::submit(self.env, &request, &bundle).map_err(error)?;
    self.patch_group(seq, &json!({"status":"running"}))?;
    Ok(request.brief)
  }

  fn relay_request(&mut self, launch: &Launch<'_>, group: &Value) -> Result<Submission> {
    let path = self.worktree_field("path").unwrap_or_default().to_owned();
    let origin = self.git(
      &argv(&["git", "-C", &path, "remote", "get-url", "origin"]),
      "git_error",
      "cannot read origin".into(),
    )?;
    let base = self.git(
      &argv(&["git", "-C", &path, "rev-parse", "HEAD"]),
      "git_error",
      "cannot read fixer HEAD".into(),
    )?;
    let brief = relay_brief(launch, group)?;
    let host = Fixer::parse(
      group
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or_default(),
    )?;
    let name = format!(
      "pb-{}-{}",
      self
        .fixer_field("round")
        .and_then(Value::as_u64)
        .unwrap_or(1),
      launch.seq
    );
    let mut command = vec![host.cli().to_owned()];
    command.extend(host.args(
      &name,
      group.get("model").and_then(Value::as_str),
      group.get("effort").and_then(Value::as_str),
      launch.unattended,
    )?);
    Ok(Submission {
      key: format!("pb-{}", toolu_state::lock::token()),
      origin,
      base,
      brief,
      agent: host.name().to_owned(),
      argv: command,
      machine: self
        .state
        .get("relayMachine")
        .and_then(Value::as_str)
        .map(str::to_owned),
      group: Some(format!("pb:{}", self.file.display())),
    })
  }

  /// Import a completed result before advancing the group.
  pub(super) fn poll_relay(&mut self, seq: u64, count: u64, remaining: u64) -> Result<bool> {
    let group = self.group(seq)?;
    let key = group
      .pointer("/relayRequest/key")
      .and_then(Value::as_str)
      .ok_or_else(|| error("missing relay key"))?;
    let task = client::get(self.env, key).map_err(error)?;
    let saved: Submission = serde_json::from_value(
      group
        .get("relayRequest")
        .cloned()
        .ok_or_else(|| error("missing relay request"))?,
    )
    .map_err(|e| error(e.to_string()))?;
    if task.request != saved {
      return Err(error("relay request differs from saved dispatch"));
    }
    match task.phase.as_str() {
      "done" => self.import_relay(seq, count, &task),
      "failed" | "cancelled" => {
        let note = task.note.as_deref().unwrap_or("relay task failed");
        let outcome = if task.phase == "failed" && super::outcome::host_limited(note) {
          "host_limited"
        } else {
          "reported_failed"
        };
        self.settle(seq, outcome, "", note)?;
        Ok(false)
      }
      "held" => {
        self.patch_group(
          seq,
          &json!({"reason": task.note.unwrap_or_else(|| "relay task held".into())}),
        )?;
        Ok(false)
      }
      "queued" | "preparing" | "running" | "cancelling" => {
        std::thread::sleep(Duration::from_secs(remaining.min(2)));
        Ok(true)
      }
      _ => Err(error(format!("unknown relay phase {}", task.phase))),
    }
  }

  fn import_relay(
    &mut self,
    seq: u64,
    count: u64,
    task: &toolu_mux::relay::store::Task,
  ) -> Result<bool> {
    let commit = task
      .result
      .as_deref()
      .ok_or_else(|| error("missing relay result"))?;
    let attempt = task
      .attempt
      .as_deref()
      .ok_or_else(|| error("missing relay attempt"))?;
    let root = self
      .worktree_field("repoRoot")
      .unwrap_or_default()
      .to_owned();
    let tree = self.worktree_field("path").unwrap_or_default().to_owned();
    super::worktree::clean_or_fail(self, &tree)?;
    let head = self.git(
      &argv(&["git", "-C", &tree, "rev-parse", "HEAD"]),
      "git_error",
      "cannot read fixer HEAD".into(),
    )?;
    if head != task.request.base && head != commit {
      return Err(error("fixer worktree changed after submission"));
    }
    let bytes = client::output(self.env, &task.request.key).map_err(error)?;
    remote::import_recover(self.env, &root, attempt, commit, &bytes)
      .map_err(|e| error(e.message))?;
    self.git(
      &argv(&["git", "-C", &tree, "merge", "--ff-only", commit]),
      "git_error",
      "cannot fast-forward fixer result".into(),
    )?;
    self.settle(seq, "done", commit, "")?;
    self.next(seq, count)
  }

  /// Wait for cancellation before removing the local worktree.
  pub(super) fn cancel_relay(&self, group: &Value) -> Result<()> {
    let key = group
      .pointer("/relayRequest/key")
      .and_then(Value::as_str)
      .ok_or_else(|| error("missing relay key"))?;
    let task = client::cancel(self.env, key).map_err(error)?;
    if !matches!(task.phase.as_str(), "cancelled" | "done" | "failed") {
      return Err(error(
        "relay cancellation pending; retry cleanup after it settles",
      ));
    }
    Ok(())
  }
}
