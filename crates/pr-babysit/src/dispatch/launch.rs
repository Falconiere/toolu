//! A fixer group's launch: its brief, then a herdr agent in the slot pane or a
//! detached `opencode run` in the slot worktree.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use super::brief::{Reports, render};
use super::dispatcher::{Dispatcher, now};
use super::herdr::{accept_claude_trust, argv, stop_agent, try_herdr};
use super::names::{agent_name, gh_config, group_file, shell_quote};
use super::opencode::{self, Run};
use super::plan::read_object;
use super::procs::stop_group;
use crate::error::Result;
use crate::fixer::host::Fixer;
use crate::route::on_path;
use crate::state::write_text;

const HERDR_TIMEOUT: Duration = Duration::from_secs(120);

const NOT_TRUST_PROMPT: &str =
  " (the blocked screen is not the standard trust prompt for this worktree; left unanswered)";

/// A herdr agent's launch.
struct Agent<'a> {
  host: Fixer,
  name: &'a str,
  pane: &'a str,
  args: &'a [String],
  message: &'a str,
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
  fn brief(&self, launch: &Launch<'_>, group: &Value, report: &Path) -> Result<String> {
    let command = format!(
      "{} babysit fixer-report {}",
      shell_quote(&self.toolu.display().to_string()),
      shell_quote(&report.display().to_string())
    );
    let done = format!("{command} done --note \"<one-line summary>\"");
    let failed = format!("{command} failed --note \"<the reason>\"");
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
    let rendered = self.brief(launch, &group, &report)?;
    let host = Fixer::parse(text(&group, "host").unwrap_or_default())?;
    let (model, effort) = (text(&group, "model"), text(&group, "effort"));
    let args = host.args(&agent, model, effort, launch.unattended)?;
    if !self.dry {
      let _absent = std::fs::remove_file(&report);
      write_text(&brief, &rendered, "brief")?;
    }
    self.patch_fixer(&json!({"current": seq}))?;
    let patch = json!({"agent": agent, "brief": brief.display().to_string(), "report": report.display().to_string(),
      "status": "launching", "reason": null, "startedAt": now()});
    self.patch_group(seq, &patch)?;
    let message = format!(
      "You are a pr-babysit fixer. Read {} and follow it exactly.",
      brief.display()
    );
    if host == Fixer::Opencode {
      let worktree = Path::new(text(launch.context, "worktree").unwrap_or_default());
      let run = Run {
        model,
        effort,
        unattended: launch.unattended,
        worktree,
        prompt: &message,
      };
      self.launch_opencode(seq, round, &run)?;
    } else {
      let started = Agent {
        host,
        name: &agent,
        pane: launch.pane,
        args: &args,
        message: &message,
      };
      self.launch_agent(seq, &started)?;
    }
    Ok(rendered)
  }

  fn launch_opencode(&mut self, seq: u64, round: u64, run: &Run<'_>) -> Result<()> {
    let mut recorded = vec!["opencode".to_owned()];
    recorded.extend(opencode::args(run)?);
    self.record(&recorded);
    if self.dry {
      return Ok(());
    }
    let log = group_file(self.file, round, seq, ".log");
    let _absent = std::fs::remove_file(&log);
    match opencode::spawn(run, &log, self.env, &gh_config(self.file)) {
      Err(error) => self.settle(seq, "agent_start_failed", "", &error),
      Ok((pid, start)) => {
        let patch = json!({"status": "running", "pid": pid, "pidStart": start, "log": log.display().to_string()});
        let recorded = self.patch_group(seq, &patch);
        if recorded.is_err() {
          stop_group(pid, &start);
        }
        recorded
      }
    }
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
    stop_agent(self.env, name);
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
