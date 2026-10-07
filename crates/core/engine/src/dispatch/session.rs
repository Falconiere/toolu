//! What stays fixed across every walk of one hook call (`sessionFor` and the
//! config switch in `dispatchHook`). Before a tool the environment gains
//! `TOOLU_CONFIG_DIR`; after it, also `PROJECT_ROOT` (the cwd's git toplevel,
//! else the cwd) and `<PROJECT_ROOT>/node_modules/.bin` first on `PATH`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;
use toolu_runtime::config::load::{WARN_PREFIX, load};
use toolu_runtime::config::read::enabled;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;

use super::{DispatchOptions, Phase};
use crate::registry::bridge::bun::BunState;

/// One hook call's fixed context.
pub(crate) struct Session<'a> {
  pub(crate) phase: Phase,
  pub(crate) host: Host,
  /// The environment every module sees.
  pub(crate) env: Env,
  /// Roots over that environment, for gating and state paths.
  pub(crate) roots: Roots,
  pub(crate) config_root: PathBuf,
  pub(crate) project_root: PathBuf,
  pub(crate) options: &'a DispatchOptions<'a>,
  /// Whether Bun was found, and whether this call already reported its absence.
  pub(crate) bun: RefCell<BunState>,
}

/// A session, or the warnings of a hook its config switched off.
pub(crate) enum Opened<'a> {
  /// `hooks.<pre|post>-tools` is false.
  Disabled(String),
  /// Ready to walk, with the warnings to print first.
  Ready(Session<'a>, String),
}

impl<'a> Session<'a> {
  /// Detects the host, loads the config and builds the module environment.
  pub(crate) fn open(phase: Phase, options: &'a DispatchOptions<'a>) -> Opened<'a> {
    let detected = detect(options.env, None);
    let host = detected.host;
    let roots = Roots::new(options.env.clone(), Some(host));
    let config = load(&roots, Some(options.cwd));
    let mut warnings: String = detected
      .warning
      .map(|line| format!("{line}\n"))
      .unwrap_or_default();
    for line in config.take_warnings() {
      warnings.push_str(WARN_PREFIX);
      warnings.push_str(&line);
      warnings.push('\n');
    }
    let name = match phase {
      Phase::Pre => "pre-tools",
      Phase::Post => "post-tools",
    };
    if !enabled(&config, "hooks", name) {
      return Opened::Disabled(warnings);
    }
    let config_root = roots.config_root();
    let config_dir = config_root.to_string_lossy().into_owned();
    let (project_root, env) = match phase {
      Phase::Pre => {
        let project = roots
          .project_root(Some(options.cwd))
          .unwrap_or_else(|| options.cwd.to_path_buf());
        (
          project,
          options.env.clone().with("TOOLU_CONFIG_DIR", &config_dir),
        )
      }
      Phase::Post => post_env(options.env, options.cwd, &config_dir),
    };
    let session = Session {
      phase,
      host,
      roots: Roots::new(env.clone(), Some(host)),
      env,
      config_root,
      project_root,
      options,
      bun: RefCell::new(BunState::default()),
    };
    Opened::Ready(session, warnings)
  }

  /// The hook process's working directory.
  pub(crate) fn cwd(&self) -> &Path {
    self.options.cwd
  }
}

/// `PROJECT_ROOT` and the post-tool environment.
fn post_env(env: &Env, cwd: &Path, config_dir: &str) -> (PathBuf, Env) {
  let project = toolu_runtime::git::toplevel(env, cwd).unwrap_or_else(|| cwd.to_path_buf());
  let root = project.to_string_lossy().into_owned();
  let path = format!(
    "{root}/node_modules/.bin:{}",
    env.get("PATH").unwrap_or_default()
  );
  let env = env
    .clone()
    .with("TOOLU_CONFIG_DIR", config_dir)
    .with("PROJECT_ROOT", &root)
    .with("PATH", &path);
  (project, env)
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
