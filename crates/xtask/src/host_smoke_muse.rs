//! Muse as a `host-smoke` driver (#552): a private `HOME` with the user's Muse
//! credential and the checkout `toolu` linked in, this checkout's two plugins
//! installed and approved there, and `muse exec` sessions in it.
//!
//! Muse forwards a hook only `HOME`, `LANG`, `LOGNAME`, `PATH`, `PWD`, `SHELL`,
//! `TERM`, `USER` and its plugin variables, never `TOOLU_BIN`. So the resolver
//! finds `toolu` in the fixed install directories, then `~/.local/bin` of that
//! private home. A `toolu` in a fixed directory would answer instead of `--bin`,
//! and the smoke refuses to run.

use std::ffi::OsString;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};

use crate::host_smoke_exec::run as run_child;
use crate::host_smoke_observe::observed_edit;
use crate::host_smoke_prompts::{CODEX_ALLOWED, CODEX_COMMIT, CODEX_PUSH, MUSE_EDIT};
use crate::host_smoke_steps::{Driver, Prompts, Run, Step};

/// A Muse installation and the checkout whose plugins it installs.
pub(crate) struct Muse {
  pub(crate) exe: PathBuf,
  pub(crate) repo: PathBuf,
}

/// The two reviewed checkout plugins this private profile installs and approves.
const PLUGINS: [&str; 2] = ["toolu", "ts-quality"];

/// The `muse exec` command line of one session; it runs in the project directory.
pub(crate) fn argv(exe: &Path, prompt: &str) -> Vec<String> {
  [
    exe.display().to_string(),
    "exec".to_owned(),
    "--json".to_owned(),
    "--approval-mode".to_owned(),
    "never".to_owned(),
    "--no-foreign-personal-context".to_owned(),
    "--disable-reminders".to_owned(),
    "--disable-web-tools".to_owned(),
    prompt.to_owned(),
  ]
  .to_vec()
}

/// The user's Muse credential, where the `muse` launcher reads it:
/// `MUSE_AUTH_PATH`, else `$XDG_CONFIG_HOME/muse/auth.json`, else
/// `$HOME/.config/muse/auth.json`, made absolute so its link resolves from the
/// private home. `var` reads the environment.
pub(crate) fn require_auth(var: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
  let auth = if let Some(path) = var("MUSE_AUTH_PATH") {
    PathBuf::from(path)
  } else if let Some(config) = var("XDG_CONFIG_HOME") {
    PathBuf::from(config).join("muse/auth.json")
  } else {
    let home = var("HOME").ok_or("HOME is not set")?;
    PathBuf::from(home).join(".config/muse/auth.json")
  };
  if auth.is_file() {
    std::fs::canonicalize(&auth).map_err(|err| format!("{}: {err}", auth.display()))
  } else {
    Err(format!(
      "no Muse auth at {}; run `muse login`",
      auth.display()
    ))
  }
}

/// `Ok` unless an executable `toolu` in one of `dirs`, searched in order before
/// `~/.local/bin`, is not `bin` (canonical); the first one found decides.
pub(crate) fn require_unshadowed(dirs: &[PathBuf], bin: &Path) -> Result<(), String> {
  for candidate in dirs.iter().map(|dir| dir.join("toolu")) {
    let executable = std::fs::metadata(&candidate)
      .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
    if !executable {
      continue;
    }
    if std::fs::canonicalize(&candidate).is_ok_and(|real| real == bin) {
      return Ok(());
    }
    return Err(format!(
      "{} would answer Muse hooks instead of {}: Muse forwards no TOOLU_BIN",
      candidate.display(),
      bin.display()
    ));
  }
  Ok(())
}

/// A private 0700 `HOME` under `root` whose `.config/muse/auth.json` links to
/// `auth` and whose `.local/bin/toolu` links to `bin`.
pub(crate) fn private_home(root: &Path, auth: &Path, bin: &Path) -> Result<PathBuf, String> {
  let home = root.join("muse-home");
  let io = |what: &Path, err: std::io::Error| format!("cannot create {}: {err}", what.display());
  std::fs::DirBuilder::new()
    .mode(0o700)
    .create(&home)
    .map_err(|err| io(&home, err))?;
  for (dir, name, target) in [
    (".config/muse", "auth.json", auth),
    (".local/bin", "toolu", bin),
  ] {
    let dir = home.join(dir);
    std::fs::create_dir_all(&dir).map_err(|err| io(&dir, err))?;
    symlink(target, dir.join(name)).map_err(|err| io(&dir.join(name), err))?;
  }
  Ok(home)
}

fn command(muse: &Muse, words: &[&str]) -> Vec<String> {
  let mut argv = vec![muse.exe.display().to_string()];
  argv.extend(words.iter().map(|word| (*word).to_owned()));
  argv
}

impl Driver for Muse {
  fn name(&self) -> &'static str {
    "muse"
  }

  fn argv(&self, step: &Step<'_>, _project: &Path) -> Vec<String> {
    argv(&self.exe, step.prompt)
  }

  fn prompts(&self) -> Prompts {
    Prompts {
      allowed: CODEX_ALLOWED,
      commit: CODEX_COMMIT,
      push: CODEX_PUSH,
      stages_first: true,
    }
  }

  /// Install each checkout plugin from its path, then approve its hooks, which
  /// stay inert until approved.
  fn prepare(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    for name in PLUGINS {
      let path = self.repo.join("plugins").join(name).display().to_string();
      let install = command(self, &["plugins", "install", &path, "--json"]);
      run_child(&install, project, run.env, None)?;
      let approve = command(self, &["plugins", "approve", name, "--json"]);
      run_child(&approve, project, run.env, None)?;
    }
    Ok(())
  }

  fn edit(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    observed_edit(run, project, &argv(&self.exe, MUSE_EDIT), "muse")
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_muse_test.rs"]
mod tests;
