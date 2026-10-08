//! The fixed commands other modules run: `git rev-parse --show-toplevel` for
//! the project root and `codex plugin list --json` for the Codex plugins.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Spec, run};
use crate::env::Env;

/// How long `command -v` and `--hook-protocol` may run.
const PROBE_TIMEOUT: Duration = Duration::from_secs(1);

/// `git rev-parse --show-toplevel` in `cwd` with exactly `env`, or `None` when
/// git is missing, fails or prints nothing.
pub fn git_toplevel(env: &Env, cwd: &Path) -> Option<PathBuf> {
  let mut spec = Spec::new(["git", "rev-parse", "--show-toplevel"]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  let output = run(&spec).ok().filter(|output| output.exit_code == 0)?;
  let top = output.stdout.trim();
  (!top.is_empty()).then(|| PathBuf::from(top))
}

/// Whether `git -C <dir> rev-parse --git-dir` succeeds with exactly `env`.
pub fn is_git_repo(env: &Env, dir: &Path) -> bool {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.extend([
    dir.display().to_string(),
    "rev-parse".to_owned(),
    "--git-dir".to_owned(),
  ]);
  spec.env = Some(env.clone());
  run(&spec).is_ok_and(|output| output.exit_code == 0)
}

/// The stdout of `codex plugin list --json` run with exactly `env`, or `None`
/// when the CLI is missing or exits non-zero.
pub fn codex_plugin_list(env: &Env) -> Option<String> {
  let mut spec = Spec::new(["codex", "plugin", "list", "--json"]);
  spec.env = Some(env.clone());
  let output = run(&spec).ok().filter(|output| output.exit_code == 0)?;
  Some(output.stdout)
}

/// What `sh -c 'command -v toolu'` resolved.
pub enum ShellToolu {
  /// A native binary: `--hook-protocol` printed a positive integer.
  Native(PathBuf),
  /// The shell found no file, or the lookup timed out.
  Missing,
  /// A file whose protocol probe is not a positive integer.
  Shadowed(PathBuf),
}

/// The native `toolu` selected by a non-login command shell of the process environment.
///
/// # Errors
/// When the shell or protocol probe cannot run, or the selected path cannot be resolved.
pub fn native_toolu_on_path() -> Result<ShellToolu, String> {
  shell_toolu(&Env::process())
}

/// The same probe as [`native_toolu_on_path`], using exactly `env`.
///
/// # Errors
/// When the shell or protocol probe cannot run, or the selected path cannot be resolved.
pub(crate) fn shell_toolu(env: &Env) -> Result<ShellToolu, String> {
  let Some(selected) = command_v(env)? else {
    return Ok(ShellToolu::Missing);
  };
  let path = std::fs::canonicalize(selected)
    .map_err(|error| format!("cannot resolve toolu from non-login shell: {error}"))?;
  if protocol_is_native(env, &path)? {
    Ok(ShellToolu::Native(path))
  } else {
    Ok(ShellToolu::Shadowed(path))
  }
}

/// The first install-path binary whose `--hook-protocol` is a positive integer.
pub(crate) fn known_native_toolu(env: &Env) -> Option<PathBuf> {
  known_candidates(env)
    .into_iter()
    .find(|path| path.is_file() && protocol_is_native(env, path).unwrap_or(false))
}

fn command_v(env: &Env) -> Result<Option<PathBuf>, String> {
  let mut shell = Spec::new(["/bin/sh", "-c", "command -v toolu"]);
  shell.timeout = PROBE_TIMEOUT;
  shell.max_output_bytes = 4096;
  shell.env = Some(env.clone());
  let found = run(&shell).map_err(|error| format!("cannot start non-login shell: {error:?}"))?;
  if found.exit_code != 0 || found.timed_out || found.truncated {
    return Ok(None);
  }
  let selected = PathBuf::from(found.stdout.trim());
  Ok(selected.is_file().then_some(selected))
}

fn known_candidates(env: &Env) -> Vec<PathBuf> {
  let mut paths = Vec::new();
  if let Some(bin) = env.get("TOOLU_BIN") {
    paths.push(absolute(PathBuf::from(bin)));
  }
  paths.extend([
    PathBuf::from("/opt/homebrew/bin/toolu"),
    PathBuf::from("/usr/local/bin/toolu"),
    PathBuf::from("/home/linuxbrew/.linuxbrew/bin/toolu"),
  ]);
  if let Some(home) = env.get("HOME") {
    paths.push(absolute(PathBuf::from(home).join(".local/bin/toolu")));
  }
  paths
}

fn absolute(path: PathBuf) -> PathBuf {
  if path.is_absolute() {
    return path;
  }
  match std::env::current_dir() {
    Ok(cwd) => cwd.join(path),
    Err(_) => path,
  }
}

fn protocol_is_native(env: &Env, path: &Path) -> Result<bool, String> {
  let program = path
    .to_str()
    .ok_or_else(|| "non-login shell returned a non-UTF-8 toolu path".to_owned())?;
  let mut probe = Spec::new([program, "--hook-protocol"]);
  probe.timeout = PROBE_TIMEOUT;
  probe.max_output_bytes = 32;
  probe.env = Some(env.clone());
  let output = run(&probe).map_err(|error| format!("cannot probe toolu: {error:?}"))?;
  Ok(
    output.exit_code == 0
      && !output.truncated
      && output
        .stdout
        .trim()
        .parse::<u32>()
        .is_ok_and(|version| version > 0),
  )
}

#[cfg(test)]
#[path = "tests/commands_test.rs"]
mod tests;
