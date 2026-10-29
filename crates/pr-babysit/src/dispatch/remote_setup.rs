//! Recoverable remote fixer setup, bound to one committed local worktree.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_mux::hc::remote::{self, Target, Transfer};

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command};
use super::remote::{fail, field, transfer};
use super::start::Start;
use super::worktree::clean_or_fail;
use crate::error::{Failure, Result};

impl Dispatcher<'_> {
  pub(super) fn resume_remote_worktree(&mut self, start: &Start<'_>) -> Result<Value> {
    let worktree = self
      .state
      .get("herdrWorktree")
      .cloned()
      .ok_or_else(|| Failure::new("state_malformed", "missing fixer worktree"))?;
    if field(&worktree, "repoRoot")? != start.repo_root
      || field(&worktree, "prBranch")? != start.branch
    {
      return Err(Failure::new(
        "remote_mismatch",
        "saved fixer worktree belongs to another repository or branch",
      ));
    }
    let path = field(&worktree, "path")?;
    let actual = self.git(
      &argv(&["git", "-C", path, "rev-parse", "--show-toplevel"]),
      "git_error",
      "cannot verify fixer worktree".into(),
    )?;
    let expected =
      std::fs::canonicalize(path).map_err(|error| Failure::new("git_error", error.to_string()))?;
    let actual = std::fs::canonicalize(actual)
      .map_err(|error| Failure::new("git_error", error.to_string()))?;
    if actual != expected {
      return Err(Failure::new(
        "remote_mismatch",
        "saved fixer path is not a worktree root",
      ));
    }
    clean_or_fail(self, path)?;
    let branch = self.git(
      &argv(&["git", "-C", path, "symbolic-ref", "--short", "HEAD"]),
      "git_error",
      "cannot read fixer branch".into(),
    )?;
    if branch != field(&worktree, "branch")? {
      return Err(Failure::new(
        "remote_mismatch",
        "saved fixer worktree branch differs",
      ));
    }
    Ok(worktree)
  }

  pub(super) fn setup_remote(
    &mut self,
    machine: &str,
    repo: &str,
    hosts: &[Value],
    base: &str,
  ) -> Result<()> {
    Target::new(Some(machine), Some(repo)).map_err(|error| fail(&error))?;
    if !self.hc_backend() {
      return Err(Failure::usage("remote fixers require herdr-app"));
    }
    if let Some(saved) = self.remote() {
      if field(saved, "machine")? != machine
        || field(saved, "repo")? != repo
        || field(saved, "baseBranch")? != base
      {
        return Err(Failure::new(
          "remote_mismatch",
          "remote target or base differs from the saved attempt",
        ));
      }
      if self.new_remote_attempt() {
        self.require_prior_stopped()?;
        self.record_remote(machine, repo, base)?;
      }
    } else {
      self.record_remote(machine, repo, base)?;
    }
    self.prepare_remote(hosts, base)
  }
}

impl Dispatcher<'_> {
  fn preflight_remote(
    &self,
    transport: &Transfer<'_>,
    hosts: &[Value],
    origin: &str,
    base: &str,
  ) -> Result<()> {
    let version = command(
      self.env,
      &[self.toolu.display().to_string(), "--version".into()],
      Duration::from_secs(30),
    );
    if version.status != 0 {
      return Err(Failure::new(
        "remote_error",
        "cannot read local toolu version",
      ));
    }
    for group in hosts {
      transport
        .preflight(origin, version.output.trim(), field(group, "host")?)
        .map_err(|error| fail(&error))?;
    }
    let root = self.worktree_field("repoRoot").unwrap_or_default();
    let reference = format!("refs/remotes/origin/{base}");
    let local = command(
      self.env,
      &argv(&["git", "-C", root, "rev-parse", "--verify", &reference]),
      Duration::from_secs(30),
    );
    let remote = transport
      .git(&["rev-parse", "--verify", &reference])
      .map_err(|error| fail(&error))?;
    if local.status != 0 || local.output.trim() != remote {
      return Err(Failure::new(
        "remote_error",
        "remote base tracking ref differs from the local base",
      ));
    }
    Ok(())
  }

  fn prepare_remote(&mut self, hosts: &[Value], base: &str) -> Result<()> {
    let saved = self
      .remote()
      .cloned()
      .ok_or_else(|| Failure::new("state_malformed", "missing remote"))?;
    let run = field(&saved, "run")?;
    let root = self
      .worktree_field("repoRoot")
      .unwrap_or_default()
      .to_owned();
    let path = self.worktree_field("path").unwrap_or_default().to_owned();
    let origin = self.git(
      &argv(&["git", "-C", &root, "remote", "get-url", "origin"]),
      "git_error",
      "cannot read local origin".into(),
    )?;
    let head = self.git(
      &argv(&["git", "-C", &path, "rev-parse", "HEAD"]),
      "git_error",
      "cannot read fixer head".into(),
    )?;
    if head != field(&saved, "expectedBase")? {
      return Err(Failure::new(
        "remote_mismatch",
        "saved fixer base commit differs",
      ));
    }
    let worktree = self.reserve_remote_path(&saved, run)?;
    let transport = transfer(self.env, &saved, run)?;
    self.preflight_remote(&transport, hosts, &origin, base)?;
    let bytes =
      remote::export_recover(self.env, &root, run, &head).map_err(|error| fail(&error))?;
    transport
      .upload("input", &head, &bytes)
      .map_err(|error| fail(&error))?;
    ensure_worktree(&transport, &worktree, &head)
  }

  fn reserve_remote_path(&mut self, saved: &Value, run: &str) -> Result<String> {
    let worktree = remote_path(&transfer(self.env, saved, run)?)?;
    if let Some(recorded) = saved.get("path").and_then(Value::as_str)
      && recorded != worktree
    {
      return Err(Failure::new(
        "remote_mismatch",
        "saved remote worktree path differs",
      ));
    }
    self.save(|state| {
      if let Some(remote) = state
        .pointer_mut("/herdrWorktree/remote")
        .and_then(Value::as_object_mut)
      {
        remote.insert("path".into(), json!(worktree));
      }
    })?;
    Ok(worktree)
  }
}

fn remote_path(transport: &Transfer<'_>) -> Result<String> {
  let file = transport.path("worktree").map_err(|error| fail(&error))?;
  Path::new(&file)
    .parent()
    .and_then(Path::to_str)
    .map(str::to_owned)
    .ok_or_else(|| Failure::new("remote_error", "invalid remote worktree path"))
}

fn registered(list: &str, path: &str, head: &str) -> Result<bool> {
  for block in list.split("\0\0") {
    let fields: Vec<&str> = block.split('\0').collect();
    if fields
      .iter()
      .any(|field| *field == format!("worktree {path}"))
    {
      if !fields.iter().any(|field| *field == format!("HEAD {head}"))
        || !fields.contains(&"detached")
      {
        return Err(Failure::new(
          "remote_mismatch",
          "saved remote worktree identity differs",
        ));
      }
      return Ok(true);
    }
  }
  Ok(false)
}

fn ensure_worktree(transport: &Transfer<'_>, path: &str, head: &str) -> Result<()> {
  let list = transport
    .git(&["worktree", "list", "--porcelain", "-z"])
    .map_err(|error| fail(&error))?;
  if !registered(&list, path, head)? {
    transport
      .git(&["worktree", "add", "--detach", path, head])
      .map_err(|error| fail(&error))?;
  }
  let list = transport
    .git(&["worktree", "list", "--porcelain", "-z"])
    .map_err(|error| fail(&error))?;
  if !registered(&list, path, head)? {
    return Err(Failure::new(
      "remote_error",
      "remote worktree was not registered",
    ));
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/remote_setup_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/remote_live_test.rs"]
mod live_tests;
