//! Remote preparation and explicit brief transport; the local mirror stays local.

use super::Ctx;
use super::attempt::Attempt;
use super::brief::{BriefPaths, prompt_text};
use super::checkout::shell_join;
use super::failure::Failure;
use super::hc::{failed, git};
use super::record::{Record, text};
use serde_json::{Value, json};
use std::path::Path;
use toolu_mux::hc::remote::{self, Target, Transfer};

fn transfer<'a>(
  record: &'a Record,
  ctx: &'a Ctx<'_>,
  run: &'a str,
) -> Result<Transfer<'a>, Failure> {
  let target = Target::new(text(record, "machine"), text(record, "remote_repo"))
    .map_err(failed)?
    .ok_or_else(|| Failure::refusal("remote target missing"))?;
  Ok(Transfer {
    target,
    env: ctx.env,
    run,
  })
}

impl Attempt<'_> {
  pub(super) fn hc_remote_prepare(
    &self,
    record: &mut Record,
    ctx: &Ctx<'_>,
  ) -> Result<(), Failure> {
    if record.get("remote_prepared") == Some(&Value::Bool(true)) {
      return Ok(());
    }
    let target = crate::hc_target::load(self.state()).map_err(Failure::refusal)?;
    if target.is_null() {
      return Ok(());
    }
    save_target(record, &target)?;
    let run = format!("{}-in", text(record, "hc_run").unwrap_or(""));
    let remote = transfer(record, ctx, &run)?;
    let origin = git(
      ctx.env,
      text(record, "checkout").unwrap_or(""),
      &["remote", "get-url", "origin"],
    )?;
    remote
      .preflight(
        &origin,
        &format!("toolu {}", env!("CARGO_PKG_VERSION")),
        self.host.kind.name(),
      )
      .map_err(failed)?;
    let repo = remote.target.repo.to_owned();
    let path = Path::new(&repo)
      .parent()
      .ok_or_else(|| Failure::refusal("remote checkout has no parent"))?
      .join(text(record, "terminal_name").unwrap_or(""))
      .display()
      .to_string();
    record.insert("remote_worktree".into(), json!(path));
    self.persist(record, "starting")?;
    self.hc_remote_input(record, ctx)?;
    record.insert("remote_prepared".into(), json!(true));
    self.persist(record, "starting")
  }

  fn hc_remote_input(&self, record: &Record, ctx: &Ctx<'_>) -> Result<(), Failure> {
    let run = format!("{}-in", text(record, "hc_run").unwrap_or(""));
    let remote = transfer(record, ctx, &run)?;
    let expected = text(record, "expected_base").unwrap_or("");
    let bytes = remote::export_recover(
      ctx.env,
      text(record, "checkout").unwrap_or(""),
      &run,
      expected,
    )
    .map_err(failed)?;
    remote.upload("input", expected, &bytes).map_err(failed)?;
    let path = text(record, "remote_worktree").unwrap_or("");
    let listed = remote
      .git(&["worktree", "list", "--porcelain", "-z"])
      .map_err(failed)?;
    if !listed
      .split('\0')
      .any(|line| line == format!("worktree {path}"))
    {
      remote
        .git(&[
          "worktree",
          "add",
          "-b",
          &self.issue.branch,
          "--",
          path,
          expected,
        ])
        .map_err(failed)?;
    }
    if remote
      .git(&["-C", path, "rev-parse", "--show-toplevel"])
      .map_err(failed)?
      != path
      || remote
        .git(&["-C", path, "symbolic-ref", "--short", "HEAD"])
        .map_err(failed)?
        != self.issue.branch
    {
      return Err(Failure::refusal("remote worktree identity differs"));
    }
    Ok(())
  }

  pub(super) fn hc_brief(
    &self,
    record: &mut Record,
    ctx: &Ctx<'_>,
    paths: &BriefPaths,
    brief: &str,
  ) -> Result<String, Failure> {
    std::fs::write(&paths.brief, brief).map_err(|e| Failure::unknown(e.to_string()))?;
    if !super::hc::remote_worker(record) {
      return Ok(prompt_text(
        paths,
        text(record, "base").unwrap_or("main"),
        self.resuming,
        false,
      ));
    }
    let run = format!("{}-in", text(record, "hc_run").unwrap_or(""));
    let remote = transfer(record, ctx, &run)?;
    let file = remote.path("brief").map_err(failed)?;
    let report = remote.path("report").map_err(failed)?;
    let message = self.remote_brief(record, ctx, &report)?;
    write_brief(&remote, &report, &message)?;
    record.insert("remote_brief".into(), json!(file));
    record.insert("remote_report".into(), json!(report));
    self.persist(record, "starting")?;
    Ok(format!(
      "Read {file} and follow this remote implementation brief exactly."
    ))
  }

  fn remote_brief(&self, record: &Record, ctx: &Ctx<'_>, report: &str) -> Result<String, Failure> {
    let kind = crate::tracker::detect_tracker(
      &self.graph.epic.reference,
      self.graph.tracker.as_deref(),
      ctx.env,
    )
    .map_err(Failure::unknown)?;
    let body = crate::tracker::issue_description(kind, &self.issue.reference, ctx.env)
      .map_err(Failure::unknown)?;
    let command = shell_join(&[
      "toolu".into(),
      "epic".into(),
      "report".into(),
      "ready".into(),
      "--status-file".into(),
      report.into(),
      "--run".into(),
      text(record, "hc_run").unwrap_or("").into(),
      "--expected-base".into(),
      text(record, "expected_base").unwrap_or("").into(),
    ]);
    Ok(format!(
      "# Remote implementation worker\n\n{}\n\n{}\n\nWork only in {} on branch {}. Implement the issue, run the applicable checks, and commit all intended changes. Do not copy credentials or local host configuration. Leave dirty work intact on failure. The initiating controller owns PR publication, review, and merge; do not push, open PRs, run review services, or merge here.\n\nWhen the clean committed result is ready, run:\n\n```text\n{command} --note '<summary>'\n```\n\nFor a failure or a question use the same command with phase failed or needs-human instead of ready. Report only full committed revisions.\n",
      self.issue.title,
      body,
      text(record, "remote_worktree").unwrap_or(""),
      self.issue.branch
    ))
  }
}

fn save_target(record: &mut Record, target: &Value) -> Result<(), Failure> {
  for key in ["machine", "remote_repo"] {
    if let Some(saved) = record.get(key)
      && Some(saved) != target.get(key)
    {
      return Err(Failure::refusal("remote run target changed"));
    }
    record.insert(
      key.into(),
      target
        .get(key)
        .cloned()
        .ok_or_else(|| Failure::refusal("remote target field missing"))?,
    );
  }
  Ok(())
}

fn write_brief(remote: &Transfer<'_>, report: &str, message: &str) -> Result<(), Failure> {
  let parent = Path::new(report)
    .parent()
    .and_then(Path::to_str)
    .ok_or_else(|| Failure::refusal("report path invalid"))?;
  remote.exec(&["mkdir", "-p", parent], &[]).map_err(failed)?;
  match remote.read("brief") {
    Ok(bytes) if bytes == message.as_bytes() => Ok(()),
    Ok(_) => Err(Failure::refusal(
      "remote brief changed; inspect dispatch before retry",
    )),
    Err(error) if error.code == "command_failed" => {
      remote.write("brief", message.as_bytes()).map_err(failed)
    }
    Err(error) => Err(failed(error)),
  }
}

#[cfg(test)]
#[path = "tests/hc_remote_test.rs"]
mod tests;
