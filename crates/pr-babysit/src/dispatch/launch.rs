//! A fixer group's launch: its brief, then a herdr agent in the slot pane.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use super::brief::{Reports, render};
use super::dispatcher::{Dispatcher, now};
use super::herdr::{accept_claude_trust, argv, stop_agent, try_herdr};
use super::names::{agent_name, group_file, shell_quote};
use super::plan::read_object;
use crate::error::Result;
use crate::fixer::host::Fixer;
use crate::route::on_path;
use crate::state::write_text;

const HERDR_TIMEOUT: Duration = Duration::from_secs(120);

const NOT_TRUST_PROMPT: &str =
  " (the blocked screen is not the standard trust prompt for this worktree; left unanswered)";

/// A herdr agent's launch.
pub(super) struct Agent<'a> {
  pub(super) host: Fixer,
  pub(super) name: &'a str,
  pub(super) pane: &'a str,
  pub(super) args: &'a [String],
  pub(super) message: &'a str,
}

/// One group's launch.
pub(super) struct Launch<'a> {
  pub(super) seq: u64,
  pub(super) items_file: &'a Path,
  pub(super) context: &'a Value,
  pub(super) pane: &'a str,
  pub(super) unattended: bool,
}

fn text<'v>(value: &'v Value, key: &str) -> Option<&'v str> {
  value.get(key).and_then(Value::as_str)
}

impl Dispatcher<'_> {
  fn brief(
    &self,
    launch: &Launch<'_>,
    group: &Value,
    report: &Path,
    remote: Option<&Value>,
  ) -> Result<String> {
    let toolu = self.toolu.display().to_string();
    let command = format!(
      "{} babysit fixer-report {}",
      shell_quote(if remote.is_some() { "toolu" } else { &toolu }),
      shell_quote(&report.display().to_string())
    );
    let done = format!("{command} done --note \"<one-line summary>\"");
    let failed = format!("{command} failed --note \"<the reason>\"");
    let binding = remote.map_or_else(String::new, |data| {
      format!(
        " --run-id {} --expected-base {} --commit \"$(git rev-parse HEAD)\"",
        shell_quote(text(data, "run").unwrap_or_default()),
        shell_quote(text(data, "base").unwrap_or_default())
      )
    });
    let done = format!("{done}{binding}");
    let failed = format!("{failed}{binding}");
    let shown = launch.items_file.display();
    let message = format!("items file is missing or not JSON: {shown}");
    let items = read_object(launch.items_file, "plan_invalid", message)?;
    let items = items
      .get("items")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    let reports = Reports {
      done: &done,
      failed: &failed,
    };
    Ok(render(&items, group, launch.context, &reports))
  }

  fn round(&self, context: &Value) -> u64 {
    let recorded = self.fixer_field("round").and_then(Value::as_u64);
    recorded
      .or_else(|| context.get("round").and_then(Value::as_u64))
      .unwrap_or(1)
  }

  /// Launches group `launch.seq` (`plan_group` in a dry run, which records
  /// nothing) and returns its brief.
  pub(super) fn launch(
    &mut self,
    launch: &Launch<'_>,
    plan_group: Option<&Value>,
  ) -> Result<String> {
    let group = match plan_group {
      Some(group) => group.clone(),
      None => self.group(launch.seq)?,
    };
    let seq = launch.seq;
    let round = self.round(launch.context);
    let agent = agent_name(text(&self.state, "slot").unwrap_or_default(), round, seq);
    let (brief, report) = (
      group_file(self.file, round, seq, ".md"),
      group_file(self.file, round, seq, ".report.json"),
    );
    let remote = if self.remote().is_some() {
      Some(self.prepare_remote_group(seq)?)
    } else {
      None
    };
    let report_path = remote
      .as_ref()
      .and_then(|data| text(data, "report"))
      .map_or(report.as_path(), Path::new);
    let rendered = self.brief(launch, &group, report_path, remote.as_ref())?;
    let host = Fixer::parse(text(&group, "host").unwrap_or_default())?;
    let (model, effort) = (text(&group, "model"), text(&group, "effort"));
    let args = host.args(&agent, model, effort, launch.unattended)?;
    let brief_path = self.brief_target(remote.as_ref(), &rendered, &brief)?;
    self.record_launch(seq, &agent, (&brief, &report), &rendered)?;
    if remote.is_some() {
      self.patch_group(seq, &json!({"brief": brief_path}))?;
    }
    let message = format!("You are a pr-babysit fixer. Read {brief_path} and follow it exactly.");
    let started = Agent {
      host,
      name: &agent,
      pane: launch.pane,
      args: &args,
      message: &message,
    };
    if self.hc_backend() {
      self.launch_hc(seq, &started)?;
    } else {
      self.launch_agent(seq, &started)?;
    }
    Ok(rendered)
  }

  fn brief_target(&self, remote: Option<&Value>, rendered: &str, brief: &Path) -> Result<String> {
    if let Some(data) = remote {
      self.write_remote_brief(data, rendered)?;
      let path = text(data, "brief").unwrap_or_default();
      return Ok(path.to_owned());
    }
    Ok(brief.display().to_string())
  }

  /// Writes the brief (not in a dry run) and marks group `seq` launching.
  fn record_launch(
    &mut self,
    seq: u64,
    agent: &str,
    (brief, report): (&Path, &Path),
    rendered: &str,
  ) -> Result<()> {
    if !self.dry {
      let _absent = std::fs::remove_file(report);
      write_text(brief, rendered, "brief")?;
    }
    self.patch_fixer(&json!({"current": seq}))?;
    let patch = json!({"agent": agent, "brief": brief.display().to_string(), "report": report.display().to_string(),
      "status": "launching", "reason": null, "startedAt": now()});
    self.patch_group(seq, &patch)
  }

  fn launch_agent(&mut self, seq: u64, agent: &Agent<'_>) -> Result<()> {
    let name = agent.name;
    let mut start = argv(&["herdr", "agent", "start", name, "--kind", agent.host.name()]);
    start.extend(argv(&["--pane", agent.pane, "--timeout", "90000", "--"]));
    start.extend_from_slice(agent.args);
    let mut prompt = argv(&["herdr", "agent", "prompt", name, agent.message, "--wait"]);
    prompt.extend(argv(&[
      "--until",
      "working",
      "--until",
      "blocked",
      "--timeout",
      "60000",
    ]));
    self.record(&start);
    self.record(&prompt);
    if self.dry {
      return Ok(());
    }
    let error = self.agent_error(agent, &start, &prompt);
    if error.is_empty() {
      return self.patch_group(seq, &json!({"status": "running"}));
    }
    stop_agent(self.env, name, Some(agent.host));
    self.settle(seq, "agent_start_failed", "", &error)
  }

  /// Why `herdr agent start` failed, unless it stopped at the trust prompt
  /// for this worktree and accepting it started the agent.
  fn start_error(&self, agent: &Agent<'_>, start: &[String]) -> Option<String> {
    let error = try_herdr(self.env, start.get(1..).unwrap_or_default(), HERDR_TIMEOUT).err()?;
    let not_ready = error.code == "agent_not_ready";
    let worktree = self.worktree_field("path").unwrap_or_default();
    if agent.host == Fixer::Claude
      && not_ready
      && accept_claude_trust(self.env, agent.name, worktree)
    {
      return None;
    }
    let note = if not_ready { NOT_TRUST_PROMPT } else { "" };
    Some(format!(
      "herdr agent start: {}: {}{note}",
      error.code, error.message
    ))
  }

  fn agent_error(&self, agent: &Agent<'_>, start: &[String], prompt: &[String]) -> String {
    let cli = agent.host.cli();
    if !on_path(self.env, cli) {
      return format!("{cli} is not on PATH");
    }
    if let Some(error) = self.start_error(agent, start) {
      return error;
    }
    match try_herdr(self.env, prompt.get(1..).unwrap_or_default(), HERDR_TIMEOUT) {
      Err(error) => format!("herdr agent prompt: {}: {}", error.code, error.message),
      Ok(_) => String::new(),
    }
  }
}

#[cfg(test)]
#[path = "tests/launch_test.rs"]
mod tests;
