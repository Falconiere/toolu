//! A hook sandbox for dispatch tests, like the TypeScript dispatch harness: a git
//! `project`, a `home` (Claude's config root is `home/.claude`), a Codex home and
//! a plugin root, a writer for `.sh` modules, and the dispatcher run over them
//! with the real environment's `PATH`.

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use toolu_engine::gate::Gate;
use toolu_engine::{DispatchOptions, Dispatched, Phase};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::Rule;

use super::sandbox::{Res, Sandbox, write};

/// One hook sandbox on a host.
pub(crate) struct Hook {
  pub(crate) sb: Sandbox,
  pub(crate) host: Host,
  /// Extra environment for the next dispatch.
  pub(crate) extra: Vec<(String, String)>,
  /// The module deadline for the next dispatch.
  pub(crate) timeout: Duration,
  /// `selected_specs` for the next dispatch.
  pub(crate) selected: Option<BTreeSet<String>>,
  /// `continue_post_blocks` for the next dispatch.
  pub(crate) continue_blocks: bool,
}

impl Hook {
  /// A sandbox whose project is a git repository.
  pub(crate) fn new(host: Host) -> Res<Hook> {
    let sb = Sandbox::new()?;
    for dir in ["project", "home", "codex", "plugin/hooks/lib"] {
      std::fs::create_dir_all(sb.path(dir)).map_err(|err| err.to_string())?;
    }
    let git = Command::new("git")
      .args(["init", "-q"])
      .current_dir(sb.path("project"))
      .status()
      .map_err(|err| err.to_string())?;
    if !git.success() {
      return Err("git init failed".to_owned());
    }
    Ok(Hook {
      sb,
      host,
      extra: Vec::new(),
      timeout: Duration::from_secs(10),
      selected: None,
      continue_blocks: false,
    })
  }

  /// The host's config root.
  pub(crate) fn config_root(&self) -> PathBuf {
    match self.host {
      Host::Codex => self.sb.path("codex"),
      Host::Claude | Host::Cursor | Host::Hermes | Host::Opencode => self.sb.path("home/.claude"),
    }
  }

  /// The event directory of `phase`.
  pub(crate) fn dir(&self, phase: Phase) -> PathBuf {
    let name = if phase == Phase::Pre {
      "pre-tools.d"
    } else {
      "post-tools.d"
    };
    self.config_root().join("toolu").join(name)
  }

  /// A `.sh` module running `body` under bash.
  pub(crate) fn sh(&self, phase: Phase, file: &str, body: &str) -> Res<()> {
    let path = self.dir(phase).join(file);
    write(&path, &format!("#!/usr/bin/env bash\n{body}\n"))?;
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&path, mode).map_err(|err| err.to_string())
  }

  /// The hook's environment: the test's `PATH`, the sandbox's home and host variables.
  pub(crate) fn env(&self) -> Env {
    let path = std::env::var("PATH").unwrap_or_default();
    let mut env = Env::from_pairs([("PATH", path), ("HOME", self.sb.text("home"))]);
    env = match self.host {
      Host::Codex => env
        .with("PLUGIN_ROOT", &self.sb.text("plugin"))
        .with("CODEX_HOME", &self.sb.text("codex")),
      Host::Claude | Host::Cursor | Host::Hermes | Host::Opencode => {
        env.with("CLAUDE_PROJECT_DIR", &self.sb.text("project"))
      }
    };
    self
      .extra
      .iter()
      .fold(env, |env, (key, value)| env.with(key, value))
  }

  /// Dispatch `stdin` for `phase` with `builtins` and `rules`.
  pub(crate) fn run(
    &self,
    phase: Phase,
    stdin: &str,
    builtins: &[&dyn Gate],
    rules: &[&dyn Rule],
  ) -> Dispatched {
    let env = self.env();
    let cwd = self.sb.path("project");
    let lib = self.sb.path("plugin/hooks/lib");
    let options = DispatchOptions {
      env: &env,
      cwd: &cwd,
      lib_dir: &lib,
      builtins,
      rules,
      selected_specs: self.selected.as_ref(),
      continue_post_blocks: self.continue_blocks,
      module_timeout: self.timeout,
    };
    toolu_engine::dispatch::dispatch(phase, stdin, &options)
  }
}
