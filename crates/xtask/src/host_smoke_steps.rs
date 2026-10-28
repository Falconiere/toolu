//! The checks `host-smoke` runs per host, the same for Claude Code, Codex and Muse: a
//! [`Driver`] says how its CLI is started, the steps say what must then hold.

use std::path::Path;

use crate::host_smoke_checks as checks;
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_exec::run as run_child;
use crate::host_smoke_project::{PushProject, git, head, nonce, remote_has, state_dir};
use crate::host_smoke_prompts::{CODEX_ALLOWED, CODEX_COMMIT, CODEX_PUSH, DENY, PUSH_REF};

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
  /// The harness stages `sample.ts` itself before the commit prompt.
  pub(crate) stages_first: bool,
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
  /// The quality-failing edit, left recorded in the gate file.
  fn edit(&self, run: &Run<'_>, project: &Path) -> Result<(), String>;
}

/// Codex's control, commit and push prompts, with `sample.ts` staged by the harness:
/// what a host gets whose shell tool is not called Bash.
pub(crate) fn staged_prompts() -> Prompts {
  Prompts {
    allowed: CODEX_ALLOWED,
    commit: CODEX_COMMIT,
    push: CODEX_PUSH,
    stages_first: true,
  }
}

/// The quality-failing edit as one session with `prompt`.
pub(crate) fn edit_session(
  driver: &dyn Driver,
  run: &Run<'_>,
  project: &Path,
  prompt: &str,
) -> Result<(), String> {
  let step = Step {
    name: "edit",
    tools: "",
    prompt,
  };
  session(driver, run, project, &step).map(drop)
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

/// The edit leaves a failing quality state.
pub(crate) fn edit(driver: &dyn Driver, run: &Run<'_>, project: &Path) -> Result<(), String> {
  driver.edit(run, project)?;
  checks::quality(project, &state_dir(driver.name()))
}

/// The commit is denied by the quality gate and `HEAD` stays.
pub(crate) fn commit(driver: &dyn Driver, run: &Run<'_>, project: &Path) -> Result<(), String> {
  let prompts = driver.prompts();
  if prompts.stages_first {
    git(project, run.env, &["add", "sample.ts"])?;
  }
  let before = head(project, run.env)?;
  let step = Step {
    name: "commit",
    tools: "Bash",
    prompt: prompts.commit,
  };
  checks::commit_denied(&session(driver, run, project, &step)?)?;
  checks::must(head(project, run.env)? == before, "the commit changed HEAD")
}

/// The push is blocked by push review and the remote gains nothing.
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
