//! Bounded Git commands used by the shape stage.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Output, Spec, run};

use super::ShapeError;

/// Maximum metadata or patch bytes retained from one Git command.
pub(super) const GIT_OUTPUT_MAX_BYTES: usize = 8 * 1024 * 1024;

/// Run Git and require complete successful output.
pub(super) fn git(env: &Env, root: &Path, args: &[&str]) -> Result<Vec<u8>, ShapeError> {
  let (stdout, truncated) = git_output(env, root, args, GIT_OUTPUT_MAX_BYTES)?;
  if truncated {
    return Err(ShapeError::Git(format!(
      "git {args:?}: output exceeds {GIT_OUTPUT_MAX_BYTES} bytes"
    )));
  }
  Ok(stdout)
}

/// A successful Git run's first `max_bytes` of output, and whether more was dropped.
pub(super) fn git_output(
  env: &Env,
  root: &Path,
  args: &[&str],
  max_bytes: usize,
) -> Result<(Vec<u8>, bool), ShapeError> {
  let out = git_process(env, root, args, max_bytes)?;
  if out.exit_code != 0 || out.timed_out {
    return Err(failure(args, &out));
  }
  Ok((out.stdout_bytes, out.truncated))
}

/// Complete output of a Git query whose exit 1 means "not found" (`None`);
/// every other failure stays a Git error.
pub(super) fn git_found(
  env: &Env,
  root: &Path,
  args: &[&str],
) -> Result<Option<Vec<u8>>, ShapeError> {
  let out = git_process(env, root, args, GIT_OUTPUT_MAX_BYTES)?;
  if out.exit_code == 1 && !out.timed_out && !out.truncated {
    return Ok(None);
  }
  if out.exit_code != 0 || out.timed_out || out.truncated {
    return Err(failure(args, &out));
  }
  Ok(Some(out.stdout_bytes))
}

/// A failed run as an error that carries Git's own message.
fn failure(args: &[&str], out: &Output) -> ShapeError {
  let stderr = out.stderr.trim();
  ShapeError::Git(format!(
    "git {args:?}: exit {}, truncated={}, timed_out={}: {stderr}",
    out.exit_code, out.truncated, out.timed_out
  ))
}

/// Run Git and retain its exit code for commands with expected nonzero results.
pub(crate) fn git_process(
  env: &Env,
  root: &Path,
  args: &[&str],
  max_bytes: usize,
) -> Result<Output, ShapeError> {
  git_process_input(env, root, args, max_bytes, Vec::new())
}

/// Run Git with supplied standard input and retain its exit code.
pub(super) fn git_process_input(
  env: &Env,
  root: &Path,
  args: &[&str],
  max_bytes: usize,
  stdin: Vec<u8>,
) -> Result<Output, ShapeError> {
  let mut spec = Spec::new(["git", "-C", &root.display().to_string()]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  spec.stdin = stdin;
  spec.max_output_bytes = max_bytes;
  run(&spec).map_err(|err| ShapeError::Git(format!("git {args:?}: {err:?}")))
}

/// Decode Git's UTF-8 text without replacing invalid bytes.
pub(super) fn text(bytes: Vec<u8>) -> Result<String, ShapeError> {
  String::from_utf8(bytes).map_err(|err| ShapeError::Git(format!("non-UTF-8 Git text: {err}")))
}

#[cfg(test)]
#[path = "tests/git_io_test.rs"]
mod tests;
