//! The checks `host-smoke` runs per host, the same for Claude Code, Codex and Muse: a
//! [`Driver`] says how its CLI is started, the steps say what must then hold.

use std::path::Path;

use crate::host_smoke_checks as checks;
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_exec::run as run_child;
use crate::host_smoke_project::{PushProject, head, nonce, remote_has, state_dir};
use crate::host_smoke_prompts::{CODEX_ALLOWED, CODEX_COMMIT, CODEX_PUSH, DENY, FAILING, PUSH_REF};

/// Where a host's transcripts go and how its children are started.
pub(crate) struct Run<'a> {
  pub(crate) root: &'a Path,
  pub(crate) env: &'a ChildEnv,
}

/// The host-specific prompts of the steps.
pub(crate) struct Prompts {
  pub(crate) allowed: &'static str,
  pub(crate) commit: &'static str,
  pub(crate) push: &'static str,
}

/// One session: a transcript name, the tools it may use, and its prompt.
pub(crate) struct Step<'a> {
  pub(crate) name: &'a str,
  pub(crate) tools: &'a str,
  pub(crate) prompt: &'a str,
}

/// A live host CLI.
pub(crate) trait Driver {
  /// `claude`, `codex` or `muse`.
  fn name(&self) -> &'static str;
  /// The argv of a session in `project`.
  fn argv(&self, step: &Step<'_>, project: &Path) -> Vec<String>;
  /// The prompts this host gets.
  fn prompts(&self) -> Prompts;
  /// Anything the host needs once, with `project` as the working directory; nothing by default.
  fn prepare(&self, _run: &Run<'_>, _project: &Path) -> Result<(), String> {
    Ok(())
  }
}

/// The control, commit and push prompts for a host whose shell tool is not Bash.
pub(crate) fn staged_prompts() -> Prompts {
  Prompts {
    allowed: CODEX_ALLOWED,
    commit: CODEX_COMMIT,
    push: CODEX_PUSH,
  }
}

/// Run `step` in `project`, keeping the transcript as `<host>-<step>.jsonl`.
pub(crate) fn session(
  driver: &dyn Driver,
  run: &Run<'_>,
  project: &Path,
  step: &Step<'_>,
) -> Result<String, String> {
  let log = run
    .root
    .join(format!("{}-{}.jsonl", driver.name(), step.name));
  run_child(&driver.argv(step, project), project, run.env, Some(&log))
}

/// A protected write is denied and the file stays.
pub(crate) fn deny(driver: &dyn Driver, run: &Run<'_>, project: &Path) -> Result<(), String> {
  let step = Step {
    name: "deny",
    tools: "Bash",
    prompt: DENY,
  };
  checks::deny(project, &session(driver, run, project, &step)?)
}

/// The clean control command runs: it prints the project's nonce.
pub(crate) fn allowed(driver: &dyn Driver, run: &Run<'_>, project: &Path) -> Result<(), String> {
  let prompt = driver.prompts().allowed;
  let step = Step {
    name: "allowed",
    tools: "Bash",
    prompt,
  };
  let expected = nonce(project)?;
  checks::ran_in(&expected, &session(driver, run, project, &step)?)
}

/// The failing quality command leaves a red gate recorded through the
/// gate-status command channel.
pub(crate) fn failing(driver: &dyn Driver, run: &Run<'_>, project: &Path) -> Result<(), String> {
  let step = Step {
    name: "failing",
    tools: "Bash",
    prompt: FAILING,
  };
  let state_dir = state_dir(driver.name());
  checks::must(
    !checks::gate(project, &state_dir).exists(),
    "quality gate existed before failing probe",
  )?;
  session(driver, run, project, &step)?;
  checks::quality(project, &state_dir)
}

/// The commit lands although the quality gate is red: enforcement is at push.
pub(crate) fn commit(driver: &dyn Driver, run: &Run<'_>, project: &Path) -> Result<(), String> {
  let state_dir = state_dir(driver.name());
  checks::quality(project, &state_dir)?;
  let before = head(project, run.env)?;
  let step = Step {
    name: "commit",
    tools: "Bash",
    prompt: driver.prompts().commit,
  };
  session(driver, run, project, &step)?;
  checks::must(
    head(project, run.env)? != before,
    "the red quality gate held the commit",
  )?;
  checks::quality(project, &state_dir)
}

/// The push is blocked while the gate is red and the remote gains nothing.
pub(crate) fn push(driver: &dyn Driver, run: &Run<'_>, pushed: &PushProject) -> Result<(), String> {
  let step = Step {
    name: "push",
    tools: "Bash",
    prompt: driver.prompts().push,
  };
  checks::push_blocked(&session(driver, run, &pushed.project, &step)?)?;
  let landed = remote_has(&pushed.remote, run.env, PUSH_REF)?;
  checks::must(!landed, "the blocked push reached the remote")
}

#[cfg(test)]
#[path = "tests/host_smoke_steps_test.rs"]
mod tests;
