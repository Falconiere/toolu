//! What an `OpenCode` worker launch needs beyond herdr (`opencode-worker.ts`):
//! a 1.x `opencode` on `PATH`, and toolu's runtime state kept out of the
//! worktree's git status through the checkout's shared `info/exclude`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use toolu_runtime::env::Env;

use super::failure::{Failure, argv, run_checked};
use crate::hosts::{opencode_model_args, opencode_version_problem};

/// toolu's `OpenCode` state inside a project: the data root and the tmp tree.
/// User-owned `.opencode` files stay visible.
pub(crate) const OPENCODE_EXCLUDE: [&str; 2] = ["/.opencode/toolu/state/", "/.opencode/tmp/"];

/// The line that opens the exclude block.
pub(crate) const EXCLUDE_MARKER: &str = "# toolu epic-orchestrator: OpenCode worker runtime state";

/// Append the marker and [`OPENCODE_EXCLUDE`] to the shared `info/exclude`
/// once; the exclude file's path.
///
/// # Errors
/// `checkout` is not in a git repository, or the file cannot be written.
pub(crate) fn exclude_opencode_state(checkout: &Path, env: &Env) -> Result<PathBuf, Failure> {
  let line = argv(&[
    "git",
    "-C",
    &checkout.display().to_string(),
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ]);
  let common = run_checked(&line, None, env, Duration::from_secs(30))?;
  let path = PathBuf::from(common.trim()).join("info").join("exclude");
  let current = std::fs::read_to_string(&path).unwrap_or_default();
  // git ignores a pattern's unescaped trailing whitespace.
  let present: Vec<&str> = current.split('\n').map(str::trim_end).collect();
  let missing: Vec<&str> = std::iter::once(EXCLUDE_MARKER)
    .chain(OPENCODE_EXCLUDE)
    .filter(|line| !present.contains(line))
    .collect();
  if missing.is_empty() {
    return Ok(path);
  }
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|err| Failure::unknown(err.to_string()))?;
  }
  let lead = if current.is_empty() || current.ends_with('\n') {
    ""
  } else {
    "\n"
  };
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&path)
    .map_err(|err| Failure::unknown(format!("{}: {err}", path.display())))?;
  file
    .write_all(format!("{lead}{}\n", missing.join("\n")).as_bytes())
    .map_err(|err| Failure::unknown(format!("{}: {err}", path.display())))?;
  Ok(path)
}

/// Exclude the worker's state (a dry run writes nothing); the launch log line.
///
/// # Errors
/// As [`exclude_opencode_state`].
pub(crate) fn exclude_for_worker(checkout: &Path, dry: bool, env: &Env) -> Result<String, Failure> {
  if !dry {
    exclude_opencode_state(checkout, env)?;
  }
  Ok(format!(
    "# opencode: exclude {} in {}'s info/exclude",
    OPENCODE_EXCLUDE.join(" "),
    checkout.display()
  ))
}

/// Refuse a model the TUI cannot take, then, on a live run, an `opencode`
/// that is missing or not a 1.x release: herdr types `opencode` into the pane.
///
/// # Errors
/// The model is not `provider/model`, or `opencode` is missing or not 1.x.
pub(crate) fn check_opencode(model: Option<&str>, dry: bool, env: &Env) -> Result<(), Failure> {
  if let Some(model) = model {
    opencode_model_args(model).map_err(Failure::refusal)?;
  }
  if dry {
    return Ok(());
  }
  let output = run_checked(&argv(&["opencode", "--version"]), None, env, Duration::from_secs(30))
    .map_err(|err| {
      let reason: String = err.message.chars().take(200).collect();
      Failure::refusal(format!(
        "opencode is not runnable here ({reason}); toolu's OpenCode plugin targets opencode-ai 1.x. Put a 1.x opencode first on PATH, or route this issue to another host."
      ))
    })?;
  match opencode_version_problem(&output) {
    Some(problem) => Err(Failure::refusal(problem)),
    None => Ok(()),
  }
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
