//! Host roots (`packages/toolu-core/src/host/host-roots.ts`): the config,
//! project, state and plugin paths, the invocation syntax and the install
//! commands of each host. Claude and Codex resolve as the bash `host.sh` did.

use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;

use super::detect::{Detected, detect};
use crate::env::Env;
use crate::process::commands::git_toplevel;

/// A caller passed an empty name where one is required (TypeScript's `TypeError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerError(pub String);

/// An environment bound to one host, detected once when not given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
  env: Env,
  host: Host,
  warning: Option<String>,
}

fn require(function: &str, field: &str, value: &str) -> Result<(), CallerError> {
  if value.is_empty() {
    Err(CallerError(format!(
      "{function}: {field} must be non-empty"
    )))
  } else {
    Ok(())
  }
}

impl Roots {
  /// `env` bound to `host`, or to the host [`detect`] finds without an event name.
  pub fn new(env: Env, host: Option<Host>) -> Roots {
    let detected = host.map_or_else(
      || detect(&env, None),
      |host| Detected {
        host,
        warning: None,
      },
    );
    let (host, warning) = (detected.host, detected.warning);
    Roots { env, host, warning }
  }

  /// The host.
  pub fn host(&self) -> Host {
    self.host
  }

  /// The environment.
  pub fn env(&self) -> &Env {
    &self.env
  }

  /// The detection warning, at most one per binding.
  pub fn warning(&self) -> Option<&str> {
    self.warning.as_deref()
  }

  /// The host's writable config and data root; `TOOLU_CONFIG_DIR` wins everywhere.
  pub fn config_root(&self) -> PathBuf {
    if let Some(dir) = self.env.get("TOOLU_CONFIG_DIR") {
      return PathBuf::from(dir);
    }
    let env = &self.env;
    let own = |key: &str, default: &str| {
      env
        .get(key)
        .map_or_else(|| env.home().join(default), PathBuf::from)
    };
    match self.host {
      Host::Claude => own("CLAUDE_CONFIG_DIR", ".claude"),
      Host::Codex => own("CODEX_HOME", ".codex"),
      Host::Cursor => env.home().join(".cursor"),
      Host::Hermes => own("HERMES_HOME", ".hermes"),
      Host::Opencode => match env.get("TOOLU_OPENCODE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => own("XDG_CONFIG_HOME", ".config").join("opencode"),
      },
    }
  }

  /// `TOOLU_PROJECT_DIR`, the host's own project variable, then the git
  /// toplevel of `cwd` (the process directory when `None`).
  pub fn project_root(&self, cwd: Option<&Path>) -> Option<PathBuf> {
    let own = match self.host {
      Host::Claude => self.env.get("CLAUDE_PROJECT_DIR"),
      Host::Cursor => self.env.get("CURSOR_PROJECT_DIR"),
      Host::Codex | Host::Hermes | Host::Opencode => None,
    };
    if let Some(dir) = self.env.get("TOOLU_PROJECT_DIR").or(own) {
      return Some(PathBuf::from(dir));
    }
    let cwd = match cwd {
      Some(cwd) => cwd.to_path_buf(),
      None => std::env::current_dir().ok()?,
    };
    git_toplevel(&self.env, &cwd)
  }

  /// `.claude`, `.codex`, `.cursor`, `.hermes` or `.opencode`, unless
  /// `TOOLU_PROJECT_CONFIG_DIRNAME` names another.
  pub fn project_dirname(&self) -> String {
    match self.env.get("TOOLU_PROJECT_CONFIG_DIRNAME") {
      Some(name) => name.to_owned(),
      None => format!(".{}", self.host.name()),
    }
  }

  /// `<project>/<dirname>/toolu.config.json`, or `None` outside a project.
  pub fn project_config_path(&self, cwd: Option<&Path>) -> Option<PathBuf> {
    let root = self.project_root(cwd)?;
    Some(root.join(self.project_dirname()).join("toolu.config.json"))
  }

  /// `<root>/<dirname>/tmp`, with `root` defaulting to the project root.
  pub fn project_state_root(&self, cwd: Option<&Path>, root: Option<&Path>) -> Option<PathBuf> {
    let root = match root {
      Some(root) => root.to_path_buf(),
      None => self.project_root(cwd)?,
    };
    Some(root.join(self.project_dirname()).join("tmp"))
  }

  /// `<state root>/<name>`.
  ///
  /// # Errors
  /// [`CallerError`] when `name` is empty.
  pub fn project_state_dir(
    &self,
    name: &str,
    cwd: Option<&Path>,
    root: Option<&Path>,
  ) -> Result<Option<PathBuf>, CallerError> {
    require("project_state_dir", "name", name)?;
    Ok(
      self
        .project_state_root(cwd, root)
        .map(|base| base.join(name)),
    )
  }

  /// The host's own plugin root variable, then `CLAUDE_PLUGIN_ROOT`.
  pub fn plugin_root(&self) -> Option<PathBuf> {
    let own = match self.host {
      Host::Codex => self.env.get("PLUGIN_ROOT"),
      Host::Cursor => self.env.get("CURSOR_PLUGIN_ROOT"),
      Host::Opencode => self.env.get("TOOLU_PLUGIN_ROOT"),
      Host::Claude | Host::Hermes => None,
    };
    own
      .or(self.env.get("CLAUDE_PLUGIN_ROOT"))
      .map(PathBuf::from)
  }

  /// `PLUGIN_DATA` on Codex, then `CLAUDE_PLUGIN_DATA`.
  pub fn plugin_data(&self) -> Option<PathBuf> {
    let own = match self.host {
      Host::Codex => self.env.get("PLUGIN_DATA"),
      Host::Claude | Host::Cursor | Host::Hermes | Host::Opencode => None,
    };
    own
      .or(self.env.get("CLAUDE_PLUGIN_DATA"))
      .map(PathBuf::from)
  }

  /// How a user invokes skill or command `name` of plugin `namespace` here.
  ///
  /// # Errors
  /// [`CallerError`] when either is empty.
  pub fn invocation(&self, namespace: &str, name: &str) -> Result<String, CallerError> {
    require("invocation", "namespace", namespace)?;
    require("invocation", "name", name)?;
    Ok(match self.host {
      Host::Codex => format!("${namespace}:{name}"),
      // Generated OpenCode commands are named `<namespace>--<name>`.
      Host::Opencode => format!("/{namespace}--{name}"),
      Host::Claude | Host::Cursor | Host::Hermes => format!("/{namespace}:{name}"),
    })
  }

  /// The host-native command that installs `spec`, or `None` where none exists yet.
  ///
  /// # Errors
  /// [`CallerError`] when `spec` is empty.
  pub fn plugin_install_command(&self, spec: &str) -> Result<Option<String>, CallerError> {
    require("plugin_install_command", "spec", spec)?;
    Ok(match self.host {
      Host::Claude => Some(format!("/plugin install {spec}")),
      Host::Codex => Some(format!("codex plugin add {spec}")),
      Host::Cursor | Host::Hermes | Host::Opencode => None,
    })
  }

  /// The command that uninstalls toolu plugin `name`, or `None` where none exists.
  ///
  /// # Errors
  /// [`CallerError`] when `name` is empty.
  pub fn plugin_uninstall_command(&self, name: &str) -> Result<Option<String>, CallerError> {
    require("plugin_uninstall_command", "name", name)?;
    Ok(match self.host {
      Host::Claude => Some(format!("claude plugin uninstall {name}@toolu")),
      Host::Codex => Some(format!("codex plugin remove {name}@toolu")),
      Host::Opencode => Some(format!(
        "npx @toolu/plugins remove {name} --host opencode --yes"
      )),
      Host::Cursor | Host::Hermes => None,
    })
  }
}

#[cfg(test)]
#[path = "tests/roots_test.rs"]
mod tests;
