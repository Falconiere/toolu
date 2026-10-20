//! Codex as a `host-smoke` driver: a private `CODEX_HOME` with this checkout's two
//! plugins installed, and `codex exec` sessions in it.

use std::os::unix::fs::{DirBuilderExt as _, symlink};
use std::path::{Path, PathBuf};

use crate::host_smoke_exec::run as run_child;
use crate::host_smoke_observe::observed_edit;
use crate::host_smoke_prompts::{CODEX_ALLOWED, CODEX_COMMIT, CODEX_EDIT, CODEX_PUSH};
use crate::host_smoke_steps::{Driver, Prompts, Run, Step};

/// A Codex installation and the checkout whose plugins it installs.
pub(crate) struct Codex {
  pub(crate) exe: PathBuf,
  pub(crate) repo: PathBuf,
}

/// The `codex exec` command line of one session in `project`.
pub(crate) fn argv(exe: &Path, project: &Path, prompt: &str) -> Vec<String> {
  [
    exe.display().to_string(),
    "--ask-for-approval".to_owned(),
    "never".to_owned(),
    "--sandbox".to_owned(),
    "workspace-write".to_owned(),
    "--dangerously-bypass-hook-trust".to_owned(),
    "exec".to_owned(),
    "--json".to_owned(),
    "--ephemeral".to_owned(),
    "-C".to_owned(),
    project.display().to_string(),
    prompt.to_owned(),
  ]
  .to_vec()
}

/// A private `CODEX_HOME` under `root` whose `auth.json` links to the user's.
pub(crate) fn private_home(root: &Path, auth: &Path) -> Result<PathBuf, String> {
  if !auth.is_file() {
    return Err(format!("no Codex auth at {}", auth.display()));
  }
  let home = root.join("codex-home");
  std::fs::DirBuilder::new()
    .mode(0o700)
    .create(&home)
    .map_err(|err| format!("cannot create {}: {err}", home.display()))?;
  symlink(auth, home.join("auth.json")).map_err(|err| format!("cannot link Codex auth: {err}"))?;
  Ok(home)
}

/// The two reviewed checkout plugins this private profile installs.
const PLUGINS: [&str; 2] = ["toolu", "ts-quality"];

fn command(codex: &Codex, words: &[&str]) -> Vec<String> {
  let mut argv = vec![codex.exe.display().to_string()];
  argv.extend(words.iter().map(|word| (*word).to_owned()));
  argv
}

impl Driver for Codex {
  fn name(&self) -> &'static str {
    "codex"
  }

  fn argv(&self, step: &Step<'_>, project: &Path) -> Vec<String> {
    argv(&self.exe, project, step.prompt)
  }

  fn prompts(&self) -> Prompts {
    Prompts {
      allowed: CODEX_ALLOWED,
      commit: CODEX_COMMIT,
      push: CODEX_PUSH,
      stages_first: true,
    }
  }

  fn prepare(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    let repo = self.repo.display().to_string();
    run_child(
      &command(self, &["plugin", "marketplace", "add", &repo]),
      project,
      run.env,
      None,
    )?;
    for name in PLUGINS {
      let spec = format!("{name}@toolu");
      run_child(
        &command(self, &["plugin", "add", &spec]),
        project,
        run.env,
        None,
      )?;
    }
    Ok(())
  }

  fn edit(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    observed_edit(run, project, &argv(&self.exe, project, CODEX_EDIT), "codex")
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_codex_test.rs"]
mod tests;
