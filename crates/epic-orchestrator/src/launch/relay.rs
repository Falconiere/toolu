//! Submit implementation work to the relay after persisting its identity.

use serde_json::{Value, json};
use toolu_mux::hc::remote;
use toolu_mux::relay::{client, store::Submission};

use super::attempt::Attempt;
use super::brief::{BriefHost, BriefPaths, render_brief};
use super::failure::Failure;
use super::record::{Record, text};
use super::{Ctx, live_result};

/// Pin relay mode before launch ownership is reserved.
pub(super) fn configure(
  record: &mut Record,
  opts: &super::LaunchOpts,
  env: &toolu_runtime::env::Env,
) -> Result<(), Failure> {
  if !opts.dry_run
    && !record.contains_key("backend")
    && !record.contains_key("stage")
    && !record.contains_key("worktree")
    && toolu_mux::relay::config::load(&toolu_mux::relay::config::root(env))
      .map_err(Failure::unknown)?
      .relay
      .is_some()
  {
    record.insert("backend".into(), json!("herdr-app"));
    record.insert("relay_enabled".into(), json!(true));
  }
  Ok(())
}

fn saved_request(record: &Record) -> Result<Option<Submission>, Failure> {
  record
    .get("relay_request")
    .map(|saved| serde_json::from_value(saved.clone()).map_err(|e| Failure::unknown(e.to_string())))
    .transpose()
}

impl Attempt<'_> {
  /// Resume the saved relay request or submit a newly persisted one.
  pub(super) fn relay_go(
    &mut self,
    record: &mut Record,
    ctx: &mut Ctx<'_>,
  ) -> Result<Value, Failure> {
    if record.get("hc_prepared") != Some(&Value::Bool(true)) {
      self.hc_prepare(record, ctx)?;
    }
    let request = if let Some(saved) = saved_request(record)? {
      saved
    } else {
      let request = self.relay_request(record, ctx)?;
      record.insert("relay_request".into(), json!(request));
      self.persist(record, "starting")?;
      request
    };
    let checkout = text(record, "checkout").unwrap_or("");
    let bundle = remote::export_recover(ctx.env, checkout, &request.key, &request.base)
      .map_err(super::hc::failed)?;
    client::submit(ctx.env, &request, &bundle).map_err(Failure::unknown)?;
    record.insert("agent".into(), json!(format!("relay:{}", request.key)));
    record.insert("prompt_state".into(), json!("acknowledged"));
    record.insert("launches".into(), json!(self.launches.max(1)));
    self.persist(record, "running")?;
    Ok(live_result(&self.issue.reference, &self.log, record))
  }

  fn relay_request(&self, record: &mut Record, ctx: &Ctx<'_>) -> Result<Submission, Failure> {
    let checkout = text(record, "checkout").unwrap_or("").to_owned();
    let base = text(record, "expected_base").unwrap_or("").to_owned();
    let key = format!("epic-{}", text(record, "hc_run").unwrap_or(""));
    let origin = super::hc::git(ctx.env, &checkout, &["remote", "get-url", "origin"])?;
    let target = crate::hc_target::load(self.state()).map_err(Failure::refusal)?;
    let machine = target
      .get("machine")
      .and_then(Value::as_str)
      .map(str::to_owned);
    let kind = crate::tracker::detect_tracker(
      &self.graph.epic.reference,
      self.graph.tracker.as_deref(),
      ctx.env,
    )
    .map_err(Failure::unknown)?;
    let body = crate::tracker::issue_description(kind, &self.issue.reference, ctx.env)
      .map_err(Failure::unknown)?;
    let epic_body = crate::tracker::issue_description(kind, &self.graph.epic.reference, ctx.env)
      .map_err(Failure::unknown)?;
    let brief = implementation_brief(self.graph, self.issue, &epic_body, &body, self.opts.safe);
    self.write_review_brief(record)?;
    Ok(Submission {
      key,
      origin,
      base,
      brief,
      agent: self.host.kind.name().into(),
      argv: super::hc::host_argv(self)?,
      machine,
      group: None,
    })
  }

  fn write_review_brief(&self, record: &mut Record) -> Result<(), Failure> {
    let paths = BriefPaths::of(
      self.state(),
      &self.issue.key,
      text(record, "worktree").unwrap_or(""),
    );
    let local = render_brief(
      self.graph,
      self.issue,
      &paths,
      text(record, "base").unwrap_or("main"),
      BriefHost::at(self.host.kind, self.opts.safe, self.state()),
    )
    .map_err(Failure::unknown)?;
    let path = std::path::Path::new(&paths.brief);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(|e| Failure::unknown(e.to_string()))?;
    }
    std::fs::write(path, local).map_err(|e| Failure::unknown(e.to_string()))?;
    record.insert("brief".into(), json!(paths.brief));
    record.insert("status_file".into(), json!(paths.status));
    Ok(())
  }
}

fn implementation_brief(
  graph: &crate::graph::Graph,
  issue: &crate::graph::GraphIssue,
  epic_body: &str,
  issue_body: &str,
  safe: bool,
) -> String {
  let blockers = issue
    .blockers
    .iter()
    .filter(|(_, state)| *state == "closed")
    .map(|(reference, _)| reference.as_str())
    .collect::<Vec<_>>();
  let blockers = if blockers.is_empty() {
    "none".to_owned()
  } else {
    blockers.join(", ")
  };
  let approvals = if safe {
    "Approval prompts may require a human decision."
  } else {
    "Work without approval prompts within these limits."
  };
  format!(
    "# Implementation: {} — {}\n\nIssue: {}\n\n{}\n\n## Epic: {} — {}\n\nEpic URL: {}\n\n{}\n\nClosed blockers: {}\n\nRead relevant blocker contracts in the machine-local repositories. Read and follow the repository's AGENTS.md or CLAUDE.md and linked quality rules. Implement only this issue, run applicable checks, and commit all intended changes. The controller handles delivery; do not push, open a PR, or merge. {} Report success only when complete; otherwise report failure using the supplied command.",
    issue.reference,
    issue.title,
    issue.url,
    issue_body,
    graph.epic.reference,
    graph.epic.title,
    graph.epic.url,
    epic_body,
    blockers,
    approvals
  )
}

#[cfg(test)]
#[path = "tests/relay_test.rs"]
mod tests;
