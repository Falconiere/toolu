//! Claude Code as a `host-smoke` driver: `claude -p` with this checkout's plugins.

use std::path::{Path, PathBuf};

use crate::host_smoke_checks::must;
use crate::host_smoke_prompts::{CLAUDE_ALLOWED, CLAUDE_COMMIT, CLAUDE_EDIT, CLAUDE_PUSH};
use crate::host_smoke_steps::{Driver, Prompts, Run, Step, session};

/// A Claude Code installation and the checkout whose plugins it loads.
pub(crate) struct Claude {
  pub(crate) exe: PathBuf,
  pub(crate) repo: PathBuf,
}

/// The `claude` command line of one session.
pub(crate) fn argv(exe: &Path, repo: &Path, step: &Step<'_>) -> Vec<String> {
  let plugin = |name: &str| repo.join("plugins").join(name).display().to_string();
  [
    exe.display().to_string(),
    "--plugin-dir".to_owned(),
    plugin("toolu"),
    "--plugin-dir".to_owned(),
    plugin("ts-quality"),
    "--model".to_owned(),
    "haiku".to_owned(),
    "-p".to_owned(),
    "--permission-mode".to_owned(),
    "auto".to_owned(),
    "--output-format".to_owned(),
    "stream-json".to_owned(),
    "--verbose".to_owned(),
    "--tools".to_owned(),
    step.tools.to_owned(),
    "--max-budget-usd".to_owned(),
    "2".to_owned(),
    step.prompt.to_owned(),
  ]
  .to_vec()
}

impl Driver for Claude {
  fn name(&self) -> &'static str {
    "claude"
  }

  fn argv(&self, step: &Step<'_>, _project: &Path) -> Vec<String> {
    argv(&self.exe, &self.repo, step)
  }

  fn prompts(&self) -> Prompts {
    Prompts {
      allowed: CLAUDE_ALLOWED,
      commit: CLAUDE_COMMIT,
      push: CLAUDE_PUSH,
      stages_first: false,
    }
  }

  fn prepare(&self, _run: &Run<'_>, _project: &Path) -> Result<(), String> {
    Ok(())
  }

  fn edit(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    let step = Step {
      name: "edit",
      tools: "Read,Edit",
      prompt: CLAUDE_EDIT,
    };
    let transcript = session(self, run, project, &step)?;
    must(
      transcript.contains("\"name\":\"Edit\""),
      "Claude did not use Edit",
    )
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_claude_test.rs"]
mod tests;
