//! `OpenCode` V2 as a `host-smoke` driver: `opencode run --standalone`, so the session
//! gets a private server that inherits this environment rather than the user's
//! shared background service. A private `XDG_CONFIG_HOME` holds the only global
//! plugins it loads: this checkout's two plugins, each a directory of links to the
//! plugin's files beside the `server.js` shim `toolu plugins install --host opencode`
//! writes. The user's own `OpenCode` plugins never load. Credentials stay in `OpenCode`'s
//! data directory, which is the user's. `TOOLU_HOST_SMOKE_OPENCODE_MODEL` picks the
//! model (`provider/model`), else `OpenCode`'s default.

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::launcher::opencode::{FILE, hooks_of, shim};

use crate::host_smoke::sweep;
use crate::host_smoke_env::ChildEnv;
use crate::host_smoke_prompts::OPENCODE_EDIT;
use crate::host_smoke_steps::{Driver, Prompts, Run, Step, edit_session, staged_prompts};

/// An `OpenCode` installation and the model its sessions use.
pub(crate) struct Opencode {
  pub(crate) exe: PathBuf,
  pub(crate) model: Option<String>,
}

/// The two checkout plugins the private config loads.
const PLUGINS: [&str; 2] = ["toolu", "ts-quality"];

/// The `opencode run` command line of one session; it runs in the project directory.
pub(crate) fn argv(exe: &Path, model: Option<&str>, prompt: &str) -> Vec<String> {
  let mut words: Vec<String> = [
    "run",
    "--standalone",
    "--auto",
    "--format",
    "json",
    "--title",
    "toolu host-smoke",
  ]
  .map(str::to_owned)
  .to_vec();
  if let Some(model) = model {
    words.extend(["--model".to_owned(), model.to_owned()]);
  }
  std::iter::once(exe.display().to_string())
    .chain(words)
    .chain([prompt.to_owned()])
    .collect()
}

/// The sweep of `exe` in a private config holding `repo`'s plugins, with the
/// model `TOOLU_HOST_SMOKE_OPENCODE_MODEL` names.
pub(crate) fn drive(exe: &Path, repo: &Path, root: &Path, env: &ChildEnv) -> Vec<String> {
  let config = match private_config(root, repo) {
    Ok(config) => config,
    Err(why) => return vec![format!("opencode: {why}")],
  };
  let driver = Opencode {
    exe: exe.to_path_buf(),
    model: std::env::var("TOOLU_HOST_SMOKE_OPENCODE_MODEL").ok(),
  };
  let env = env.with_opencode_config(&config);
  sweep(&driver, &Run { root, env: &env })
}

/// `<root>/opencode-config`, the private `XDG_CONFIG_HOME` whose
/// `opencode/plugins/` holds each checkout plugin of `repo` with its shim.
pub(crate) fn private_config(root: &Path, repo: &Path) -> Result<PathBuf, String> {
  let config = root.join("opencode-config");
  for name in PLUGINS {
    let source = repo.join("plugins").join(name);
    let target = config.join("opencode/plugins").join(name);
    link_plugin(&source, &target, name)?;
  }
  Ok(config)
}

/// `target` as links to every entry of `source`, plus the generated shim.
fn link_plugin(source: &Path, target: &Path, name: &str) -> Result<(), String> {
  let io = |what: &str, path: &Path, err: std::io::Error| {
    format!("cannot {what} {}: {err}", path.display())
  };
  std::fs::create_dir_all(target).map_err(|err| io("create", target, err))?;
  let entries = std::fs::read_dir(source).map_err(|err| io("list", source, err))?;
  for entry in entries {
    let entry = entry.map_err(|err| io("list", source, err))?;
    let link = target.join(entry.file_name());
    symlink(entry.path(), &link).map_err(|err| io("link", &link, err))?;
  }
  let hooks_path = source.join("hooks/hooks.json");
  let text = std::fs::read_to_string(&hooks_path).map_err(|err| io("read", &hooks_path, err))?;
  let hooks: Value = serde_json::from_str(&text)
    .map_err(|err| format!("{} is not JSON: {err}", hooks_path.display()))?;
  let catalog = ["skills", "commands"]
    .iter()
    .any(|dir| source.join(dir).is_dir());
  let file = target.join(FILE);
  std::fs::write(&file, shim(name, &hooks_of(&hooks), catalog)?)
    .map_err(|err| io("write", &file, err))
}

impl Driver for Opencode {
  fn name(&self) -> &'static str {
    "opencode"
  }

  fn argv(&self, step: &Step<'_>, _project: &Path) -> Vec<String> {
    argv(&self.exe, self.model.as_deref(), step.prompt)
  }

  fn prompts(&self) -> Prompts {
    staged_prompts()
  }

  fn edit(&self, run: &Run<'_>, project: &Path) -> Result<(), String> {
    edit_session(self, run, project, OPENCODE_EDIT)
  }
}

#[cfg(test)]
#[path = "tests/host_smoke_opencode_test.rs"]
mod tests;
