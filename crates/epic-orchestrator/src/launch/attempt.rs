//! One launch attempt after the refusals: reserve, initialize the record,
//! prepare the checkout and worktree, start the agent, write the brief and
//! prompt it, then record the acknowledged outcome.

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};

use super::admission::{Owner, Reserve, launch_reservation, persist_attempt};
use super::agent::{AgentPlan, AgentStart, Resolved, ensure_agent};
use super::checkout::prepare_checkout;
use super::failure::Failure;
use super::opencode::exclude_for_worker;
use super::record::{Record, text};
use super::session::capture_session;
use super::worktree::{Place, Worktree, ensure_worktree};
use super::{Ctx, LaunchOpts};
use crate::graph::{Graph, GraphIssue};
use crate::hosts::HostKind;

/// The attempt's inputs and what it has done so far.
pub(super) struct Attempt<'g> {
  pub(super) graph: &'g Graph,
  pub(super) issue: &'g GraphIssue,
  pub(super) opts: &'g LaunchOpts,
  pub(super) host: Resolved,
  pub(super) started_at: u64,
  /// The issue has a record or is in flight.
  pub resuming: bool,
  /// The record's host before this attempt.
  pub previous: Option<String>,
  /// Successful agent starts before this attempt.
  pub launches: u64,
  pub(super) owner: Option<Owner>,
  pub(super) log: Vec<String>,
}

fn iso_millis(epoch_ms: u64) -> String {
  toolu_state::time::iso_millis(UNIX_EPOCH + Duration::from_millis(epoch_ms))
}

impl<'g> Attempt<'g> {
  /// An attempt that began at `started_at` (epoch ms).
  pub(super) fn new(
    graph: &'g Graph,
    issue: &'g GraphIssue,
    opts: &'g LaunchOpts,
    host: Resolved,
    started_at: u64,
  ) -> Self {
    Self {
      graph,
      issue,
      opts,
      host,
      started_at,
      resuming: false,
      previous: None,
      launches: 0,
      owner: None,
      log: Vec::new(),
    }
  }

  pub(super) fn state(&self) -> &Path {
    Path::new(&self.graph.state_dir)
  }

  pub(super) fn persist(&self, record: &mut Record, stage: &str) -> Result<(), Failure> {
    match &self.owner {
      Some(owner) => persist_attempt(self.state(), &self.issue.key, owner, record, stage),
      None => Ok(()),
    }
  }

  /// Reserve the lease (not on a dry run) and record the attempt as `starting`.
  ///
  /// # Errors
  /// The reservation is refused.
  pub(super) fn reserve(&mut self, record: &mut Record, ctx: &mut Ctx<'_>) -> Result<(), Failure> {
    if self.opts.dry_run {
      return Ok(());
    }
    let input = Reserve {
      state: Path::new(&self.graph.state_dir),
      key: &self.issue.key,
      host: self.host.kind,
      max_parallel: self.graph.max_parallel,
      replace: self.opts.replace(),
      env: ctx.env,
    };
    self.owner = Some(launch_reservation(&input, record, &mut ctx.herdr)?);
    Ok(())
  }

  /// Leave a failed reserved attempt `uncertain` with its failure details.
  ///
  /// # Errors
  /// `outcome`'s failure, or the record cannot be written.
  pub(super) fn settle(
    &self,
    record: &mut Record,
    outcome: Result<Value, Failure>,
  ) -> Result<Value, Failure> {
    let Err(err) = outcome else {
      return outcome;
    };
    if self.owner.is_some() {
      record.extend(err.details());
      self.persist(record, "uncertain")?;
    }
    Err(err)
  }

  pub(super) fn initialize(&self, record: &mut Record) -> Result<(), Failure> {
    let issue = self.issue;
    for (key, value) in [
      ("ref", json!(issue.reference)),
      ("key", json!(issue.key)),
      ("repo", json!(issue.repo)),
      ("number", json!(issue.number)),
      ("url", json!(issue.url)),
      ("title", json!(issue.title)),
      ("branch", json!(issue.branch)),
      ("kind", json!(self.host.kind.name())),
      ("prompt_state", json!("pending")),
      ("attempt_started_at", json!(iso_millis(self.started_at))),
    ] {
      record.insert(key.to_owned(), value);
    }
    self.persist(record, "starting")
  }

  pub(super) fn prepare(
    &mut self,
    record: &mut Record,
    ctx: &mut Ctx<'_>,
  ) -> Result<(Worktree, String), Failure> {
    let dry = self.opts.dry_run;
    let (checkout, base) = prepare_checkout(
      self.graph,
      self.issue,
      dry,
      &mut self.log,
      (ctx.env, ctx.gh.clone()),
    )?;
    let place = Place {
      checkout: &checkout,
      base: &base,
      dry,
    };
    let wt = ensure_worktree(&mut ctx.herdr, &place, self.issue, &mut self.log)?;
    if self.host.kind == HostKind::Opencode {
      self
        .log
        .push(exclude_for_worker(Path::new(&checkout), dry, ctx.env)?);
    }
    if let Some(owner) = &self.owner {
      for (key, value) in [
        ("workspace_id", &wt.workspace_id),
        ("pane_id", &wt.pane_id),
        ("worktree", &wt.path),
        ("checkout", &checkout),
        ("base", &base),
        ("agent", &self.issue.key),
      ] {
        record.insert(key.to_owned(), json!(value));
      }
      let state = self.graph.state_dir.as_str();
      toolu_engine::resources::binding::bind_worktree(
        &owner.root,
        Path::new(&wt.path),
        &self.issue.key,
        state,
      )
      .map_err(Failure::unknown)?;
      self.persist(record, "starting")?;
    }
    Ok((wt, base))
  }

  pub(super) fn start(
    &mut self,
    record: &mut Record,
    wt: &Worktree,
    ctx: &mut Ctx<'_>,
  ) -> Result<AgentStart, Failure> {
    let plan = AgentPlan {
      key: self.issue.key.clone(),
      pane: wt.pane_id.clone(),
      dry: self.opts.dry_run,
      replace: self.opts.replace(),
      host: self.host.clone(),
      bypass: !self.opts.safe,
      permission_mode: self.opts.permission_mode.clone(),
      previous_kind: self.previous.clone(),
      launches: self.launches,
      worktree: wt.path.clone(),
      session_id: text(record, "session_id").map(str::to_owned),
      session_started_after: record.get("session_started_after").and_then(Value::as_u64),
    };
    let start = ensure_agent(&mut ctx.herdr, &plan, &mut self.log)?;
    if !self.opts.dry_run && start.started && !start.resumed {
      record.insert("session_started_after".to_owned(), json!(self.started_at));
      let id = capture_session(self.host.kind, &wt.path, self.started_at, ctx.env)?;
      record.insert("session_id".to_owned(), json!(id));
    }
    Ok(start)
  }
}

#[cfg(test)]
#[path = "tests/attempt_test.rs"]
mod tests;
