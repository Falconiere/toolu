//! The rest of a launch attempt: the brief, the dry-run document, the prompt,
//! the session and the acknowledged outcome written to the record.

use std::path::Path;
use std::time::SystemTime;

use serde_json::{Value, json};

use super::agent::AgentStart;
use super::attempt::Attempt;
use super::brief::{BriefHost, BriefPaths, prompt_text, render_brief};
use super::checkout::shell_join;
use super::failure::Failure;
use super::lifecycle::{Expected, Outcome, acknowledged_outcome};
use super::record::Record;
use super::session::await_session;
use super::worktree::Worktree;
use super::{Ctx, live_result};
use crate::herdr::Call;

impl Attempt<'_> {
  fn dry_document(
    &self,
    start: &AgentStart,
    paths: &BriefPaths,
    prompt: &str,
    brief: &str,
  ) -> Value {
    json!({
      "dry_run": true,
      "issue": self.issue.reference,
      "host": {"kind": self.host.kind.name(), "model": self.host.model, "effort": self.host.effort},
      "commands": self.log,
      "agent_args": start.args,
      "resume": start.resumed,
      "prompt": prompt,
      "brief_path": paths.brief,
      "brief": brief,
    })
  }

  /// Run the attempt.
  ///
  /// # Errors
  /// Any step fails.
  pub(super) fn go(&mut self, record: &mut Record, ctx: &mut Ctx<'_>) -> Result<Value, Failure> {
    self.initialize(record)?;
    let (wt, base) = self.prepare(record, ctx)?;
    let start = self.start(record, &wt, ctx)?;
    let paths = BriefPaths::of(self.state(), &self.issue.key, &wt.path);
    let host = BriefHost {
      kind: self.host.kind,
      safe: self.opts.safe,
    };
    let brief =
      render_brief(self.graph, self.issue, &paths, &base, host).map_err(Failure::unknown)?;
    let prompted = start.started || self.opts.reprompt();
    let text = prompt_text(&paths, &base, self.resuming, start.resumed);
    let prompt = Call::PromptWait {
      target: self.issue.key.clone(),
      text: text.clone(),
      until: vec!["working".to_owned(), "blocked".to_owned()],
      timeout_ms: 60_000,
    };
    if prompted {
      self.log.push(shell_join(&prompt.argv(ctx.env)));
    }
    if self.opts.dry_run {
      return Ok(self.dry_document(&start, &paths, &text, &brief));
    }
    write_brief(&paths.brief, &brief)?;
    if self.owner.is_some() {
      record.insert("brief".to_owned(), json!(paths.brief));
      record.insert("status_file".to_owned(), json!(paths.status));
      record.insert(
        "prompt_state".to_owned(),
        json!(if prompted { "submitting" } else { "retained" }),
      );
      self.persist(record, "starting")?;
    }
    self.deliver(record, ctx, (&wt, &paths, &start), &prompt)
  }

  fn deliver(
    &mut self,
    record: &mut Record,
    ctx: &mut Ctx<'_>,
    done: (&Worktree, &BriefPaths, &AgentStart),
    prompt: &Call,
  ) -> Result<Value, Failure> {
    let (wt, paths, start) = done;
    let prompted = start.started || self.opts.reprompt();
    if prompted {
      ctx.herdr.call(prompt)?;
    }
    if start.started && !start.resumed {
      let id = await_session(
        self.host.kind,
        &wt.path,
        self.started_at,
        ctx.env,
        ctx.session_wait,
      )?;
      record.insert("session_id".to_owned(), json!(id));
    }
    let fallback = match (start.resumed, start.started) {
      (true, _) => Outcome::Resumed,
      (false, true) => Outcome::Started,
      (false, false) => Outcome::Retained,
    };
    let expected = Expected {
      kind: self.host.kind,
      pane: &wt.pane_id,
      cwd: &wt.path,
    };
    let outcome = acknowledged_outcome(&mut ctx.herdr, &self.issue.key, &expected, fallback)?;
    record.insert("bypass".to_owned(), json!(!self.opts.safe));
    self.finish_record(record, paths, (start.started, prompted), outcome);
    self.persist(record, "running")?;
    Ok(live_result(&self.issue.reference, &self.log, record))
  }

  fn finish_record(
    &self,
    record: &mut Record,
    paths: &BriefPaths,
    done: (bool, bool),
    outcome: Outcome,
  ) {
    let (started, prompted) = done;
    let now = toolu_state::time::iso_seconds(SystemTime::now());
    let launched_at = record
      .get("launched_at")
      .cloned()
      .unwrap_or_else(|| json!(now));
    let last_launch = if prompted {
      json!(now)
    } else {
      record.get("last_launch").cloned().unwrap_or(Value::Null)
    };
    let launches = if started {
      self.launches + 1
    } else {
      self.launches
    };
    for (key, value) in [
      ("model", json!(self.host.model)),
      ("effort", json!(self.host.effort)),
      ("stage", json!("running")),
      ("status_file", json!(paths.status)),
      ("brief", json!(paths.brief)),
      ("launched_at", launched_at),
      ("last_launch", last_launch),
      ("launches", json!(launches)),
      ("prompt_state", json!("acknowledged")),
      ("lifecycle_outcome", json!(outcome.name())),
    ] {
      record.insert(key.to_owned(), value);
    }
  }
}

fn write_brief(path: &str, brief: &str) -> Result<(), Failure> {
  let path = Path::new(path);
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)
      .map_err(|err| Failure::unknown(format!("{}: {err}", parent.display())))?;
  }
  std::fs::write(path, brief).map_err(|err| Failure::unknown(format!("{}: {err}", path.display())))
}

#[cfg(test)]
#[path = "tests/deliver_test.rs"]
mod tests;
