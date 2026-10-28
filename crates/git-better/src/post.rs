//! The `commit-note` post-tool rule: after a Bash/Shell command that ran `git
//! commit`, attach the provenance note to the commit it made. It writes only
//! when HEAD's newest reflog entry is a commit from the last two minutes, so a
//! failed commit never labels an older one, and never changes the decision.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_shell::git::git_invocation;
use toolu_state::git::{base_branch, current_branch};

use crate::SPEC;
use crate::gitio::git;
use crate::note::{Provenance, write};
use crate::settings;

/// The rule's manifest name.
pub(crate) const NAME: &str = "commit-note";
/// How recent the commit must be.
const FRESH_SECONDS: u64 = 120;

/// The compiled post-tool rule.
struct CommitNote;

/// `commit-note` manifest's compiled rule.
pub const COMMIT_NOTE: &dyn Rule = &CommitNote;

fn command_of(event: &NormalizedEvent) -> Option<&str> {
  event
    .tool()
    .and_then(|tool| tool.input.get("command"))
    .and_then(Value::as_str)
}

/// The directory of the first static `git commit` in `command`.
fn commit_dir(command: &str, cwd: &Path) -> Option<PathBuf> {
  let analysis = toolu_shell::analyze(command);
  let commit = analysis
    .commands
    .iter()
    .filter_map(git_invocation)
    .find(|git| git.subcommand == Some("commit"))?;
  let mut dir = cwd.to_path_buf();
  for step in &commit.c_chain {
    dir = dir.join(step.as_deref()?);
  }
  Some(dir)
}

/// The host reported a non-zero exit for the command.
fn failed(event: &NormalizedEvent) -> bool {
  let NormalizedEvent::ToolPost {
    output: Some(output),
    ..
  } = event
  else {
    return false;
  };
  ["exit_code", "exitCode"]
    .iter()
    .filter_map(|key| output.get(key).and_then(Value::as_i64))
    .any(|code| code != 0)
}

/// HEAD's commit when its newest reflog entry is a commit made just now.
fn fresh_commit(env: &Env, dir: &Path) -> Option<String> {
  let args = [
    "log",
    "-g",
    "-1",
    "--format=%H%x1f%gs%x1f%gd",
    "--date=unix",
    "HEAD",
  ];
  let line = git(env, dir, &args, &[]).ok()?;
  let mut fields = line.trim_end().split('\x1f');
  let (sha, subject, selector) = (fields.next()?, fields.next()?, fields.next()?);
  let seconds: u64 = selector
    .strip_prefix("HEAD@{")?
    .strip_suffix('}')?
    .parse()
    .ok()?;
  let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
  let recent = now.saturating_sub(seconds) <= FRESH_SECONDS;
  (subject.starts_with("commit") && recent).then(|| sha.to_owned())
}

/// Attach the note to the commit the command just made, when it made one.
///
/// # Errors
/// When git cannot read the commit or store its note.
fn note_after(event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<(), String> {
  let cwd = ctx.cwd.unwrap_or(ctx.project_root);
  let Some(dir) = command_of(event).and_then(|command| commit_dir(command, cwd)) else {
    return Ok(());
  };
  if failed(event) {
    return Ok(());
  }
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let settings = settings::load(&roots, Some(&dir), ctx.plugin_root);
  let commit = fresh_commit(ctx.env, &dir).filter(|_| settings.write_notes);
  let Some(commit) = commit else {
    return Ok(());
  };
  let provenance = Provenance {
    branch: current_branch(ctx.env, &dir),
    base: base_branch(ctx.env, None, &dir),
    host: ctx.host.name().to_owned(),
    session: event.session().session_id.as_str().to_owned(),
  };
  write(ctx.env, &dir, &commit, &provenance, &settings.area_roots).map(drop)
}

impl Rule for CommitNote {
  fn spec(&self) -> &'static str {
    SPEC
  }
  fn name(&self) -> &'static str {
    NAME
  }
  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPost
  }

  fn applies(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    let shell = event
      .tool()
      .is_some_and(|tool| matches!(tool.name.as_str(), "Bash" | "Shell"));
    shell && command_of(event).is_some_and(|command| command.contains("commit"))
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    note_after(event, ctx).unwrap_or_default();
    Decision::Allow
  }

  fn run_warning(
    &self,
    event: &NormalizedEvent,
    ctx: &RuleContext<'_>,
    warnings: &mut Vec<String>,
  ) -> Decision {
    if let Err(error) = note_after(event, ctx) {
      warnings.push(format!(
        "git-better could not write the commit note: {error}"
      ));
    }
    Decision::Allow
  }
}

#[cfg(test)]
#[path = "tests/post_test.rs"]
mod tests;
