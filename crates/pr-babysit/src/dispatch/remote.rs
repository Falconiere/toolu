//! Remote fixer worktrees and explicit committed transfers.

use std::time::Duration;

use serde_json::{Value, json};
use toolu_mux::hc::remote::{self, Target, Transfer};

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command};
use super::remote_finish::{verify_remote_result, verify_reported_commit};
use crate::error::{Failure, Result};

pub(super) fn fail(error: &toolu_mux::hc::Error) -> Failure {
  Failure::new("remote_error", format!("{}: {}", error.code, error.message))
    .with("uncertain", error.uncertain)
}

pub(super) fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
  value
    .get(name)
    .and_then(Value::as_str)
    .filter(|s| !s.is_empty())
    .ok_or_else(|| Failure::new("state_malformed", format!("missing remote {name}")))
}

pub(super) fn transfer<'a>(
  env: &'a toolu_runtime::env::Env,
  state: &'a Value,
  run: &'a str,
) -> Result<Transfer<'a>> {
  let target = Target::new(Some(field(state, "machine")?), Some(field(state, "repo")?))
    .map_err(|error| fail(&error))?
    .ok_or_else(|| Failure::new("state_malformed", "missing remote target"))?;
  Ok(Transfer { target, env, run })
}

impl Dispatcher<'_> {
  pub(super) fn remote(&self) -> Option<&Value> {
    self
      .state
      .pointer("/herdrWorktree/remote")
      .filter(|v| v.is_object())
  }
}

#[cfg(test)]
#[path = "tests/remote_test.rs"]
mod tests;

pub(super) enum RemoteOutcome {
  Done(String),
  Failed(String),
}

impl Dispatcher<'_> {
  pub(super) fn prepare_remote_group(&mut self, seq: u64) -> Result<Value> {
    let remote = self
      .remote()
      .cloned()
      .ok_or_else(|| Failure::new("state_malformed", "missing remote"))?;
    let run = format!("{}-g{seq}", field(&remote, "run")?);
    let transport = transfer(self.env, &remote, &run)?;
    let path = field(&remote, "path")?;
    let base = transport
      .git(&["-C", path, "rev-parse", "HEAD"])
      .map_err(|error| fail(&error))?;
    let brief = transport.path("brief").map_err(|error| fail(&error))?;
    let report = transport.path("report").map_err(|error| fail(&error))?;
    let data = json!({"run": run, "base": base, "brief": brief, "report": report});
    if let Some(previous) = self.group(seq)?.get("remote") {
      if previous != &data {
        return Err(Failure::new(
          "remote_mismatch",
          "saved remote group identity differs",
        ));
      }
    } else {
      self.patch_group(seq, &json!({"remote": data}))?;
    }
    let transport = transfer(self.env, &remote, &run)?;
    let dir = std::path::Path::new(&report)
      .parent()
      .and_then(std::path::Path::to_str)
      .ok_or_else(|| Failure::new("remote_error", "invalid report path"))?;
    transport
      .exec(&["mkdir", "-p", dir], &[])
      .map_err(|error| fail(&error))?;
    Ok(data)
  }

  pub(super) fn write_remote_brief(&self, data: &Value, body: &str) -> Result<()> {
    let remote = self
      .remote()
      .ok_or_else(|| Failure::new("state_malformed", "missing remote"))?;
    let transport = transfer(self.env, remote, field(data, "run")?)?;
    match transport.write("brief", body.as_bytes()) {
      Ok(()) => Ok(()),
      Err(_error)
        if transport
          .read("brief")
          .is_ok_and(|saved| saved == body.as_bytes()) =>
      {
        Ok(())
      }
      Err(error) => Err(fail(&error)),
    }
  }

  pub(super) fn remote_result(&mut self, seq: u64, group: &Value) -> Result<RemoteOutcome> {
    let remote = self
      .remote()
      .cloned()
      .ok_or_else(|| Failure::new("state_malformed", "missing remote"))?;
    let data = group
      .get("remote")
      .ok_or_else(|| Failure::new("state_malformed", "missing remote group"))?;
    if let Some(commit) = group.get("remoteImported").and_then(Value::as_str) {
      return self.confirm_imported(commit).map(RemoteOutcome::Done);
    }
    let run = field(data, "run")?;
    let transport = transfer(self.env, &remote, run)?;
    let bytes = transport.read("report").map_err(|error| fail(&error))?;
    let report: Value = serde_json::from_slice(&bytes)
      .map_err(|_error| Failure::new("remote_error", "invalid remote report"))?;
    let commit = validate_report(&report, data)?.to_owned();
    if report.get("status").and_then(Value::as_str) == Some("failed") {
      verify_reported_commit(&transport, &remote, data, &commit)?;
      let note = report
        .get("note")
        .and_then(Value::as_str)
        .unwrap_or_default();
      return Ok(RemoteOutcome::Failed(note.to_owned()));
    }
    verify_remote_result(&transport, &remote, data, &commit)?;
    self.patch_group(seq, &json!({"remoteExportStarted": true}))?;
    self.import_remote_result(seq, &remote, run, &commit)?;
    Ok(RemoteOutcome::Done(commit))
  }
  fn confirm_imported(&self, commit: &str) -> Result<String> {
    let local = self.worktree_field("path").unwrap_or_default();
    let head = command(
      self.env,
      &argv(&["git", "-C", local, "rev-parse", "HEAD"]),
      Duration::from_secs(30),
    );
    if head.status == 0 && head.output.trim() == commit {
      return Ok(commit.to_owned());
    }
    Err(Failure::new(
      "remote_error",
      "local fixer head changed after import",
    ))
  }

  fn import_remote_result(
    &mut self,
    seq: u64,
    remote: &Value,
    run: &str,
    commit: &str,
  ) -> Result<()> {
    let transport = transfer(self.env, remote, run)?;
    let bundle = transport
      .export_recover("output", commit)
      .map_err(|error| fail(&error))?;
    let root = self
      .worktree_field("repoRoot")
      .unwrap_or_default()
      .to_owned();
    remote::import_recover(self.env, &root, run, commit, &bundle).map_err(|error| fail(&error))?;
    let local = self.worktree_field("path").unwrap_or_default().to_owned();
    self.git(
      &argv(&["git", "-C", &local, "merge", "--ff-only", commit]),
      "git_error",
      "could not fast-forward local fixer worktree".into(),
    )?;
    self.patch_group(seq, &json!({"remoteImported": commit}))?;
    Ok(())
  }
}

fn validate_report<'a>(report: &'a Value, expected: &Value) -> Result<&'a str> {
  let run = field(expected, "run")?;
  let base = field(expected, "base")?;
  if report.get("version").and_then(Value::as_u64) != Some(1)
    || !matches!(
      report.get("status").and_then(Value::as_str),
      Some("done" | "failed")
    )
    || report.get("runId").and_then(Value::as_str) != Some(run)
    || report.get("expectedBase").and_then(Value::as_str) != Some(base)
  {
    return Err(Failure::new(
      "remote_error",
      "remote report identity or status differs",
    ));
  }
  let commit = field(report, "commit")?;
  if !matches!(commit.len(), 40 | 64)
    || !commit
      .bytes()
      .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
  {
    return Err(Failure::new("remote_error", "invalid reported commit"));
  }
  Ok(commit)
}
