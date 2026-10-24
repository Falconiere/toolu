//! Cursor Agent as a `host-smoke` driver (#589): `cursor-agent -p` in the user's own
//! profile (its login lives in the keychain, so there is no private home), with this
//! checkout's two plugins loaded by `--plugin-dir`. Cursor runs the plugins' Claude-format
//! hooks, which is what this proves.

use std::path::{Path, PathBuf};

use crate::host_smoke_prompts::{CODEX_ALLOWED, CODEX_COMMIT, CODEX_PUSH, CURSOR_EDIT};
use crate::host_smoke_steps::{Driver, Prompts, Run, Step, session};

/// A Cursor Agent installation and the checkout whose plugins it loads.
pub(crate) struct Cursor {
  pub(crate) exe: PathBuf,
  pub(crate) repo: PathBuf,
}

/// The `cursor-agent` command line of one session.
pub(crate) fn argv(exe: &Path, repo: &Path, prompt: &str) -> Vec<String> {
  let plugin = |name: &str| repo.join("plugins").join(name).display().to_string();
  [
    exe.display().to_string(),
    "-p".to_owned(),
    "--trust".to_owned(),
    "--force".to_owned(),
    "--plugin-dir".to_owned(),
    plugin("toolu"),
    "--plugin-dir".to_owned(),
    plugin("ts-quality"),
    "--output-format".to_owned(),
    "stream-json".to_owned(),
    prompt.to_owned(),
  ]
  .to_vec()
}

impl Driver for Cursor {
  fn name(&self) -> &'static str {
    "cursor"
  }

  fn argv(&self, step: &Step<'_>, _project: &Path) -> Vec<String> {
    argv(&self.exe, &self.repo, step.prompt)
  }

  fn prompts(&self) -> Prompts {
    Prompts {
      allowed: CODEX_ALLOWED,
      commit: CODEX_COMMIT,
      push: CODEX_PUSH,
      stages_first: true,
    }
  }

  fn edit(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    let step = Step {
      name: "edit",
      tools: "",
      prompt: CURSOR_EDIT,
    };
    session(self, run, project, &step).map(drop)
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_cursor_test.rs"]
mod tests;
