//! Replace this process with another program (`execvp`): the program keeps this
//! process's pid, process group, controlling terminal and standard streams, so its
//! output, its signals and its exit status are the caller's own. A verb that hands
//! the terminal to a command (`toolu sandbox run`, `toolu sandbox shell`) ends here;
//! nothing after a successful call runs, so the caller holds no lock or file then.
//!
//! The program is found on `PATH` (`/usr/bin:/bin` when it is unset, as `execvp`
//! does) before anything else happens. std's `exec`
//! resets SIGPIPE and the signal mask of this process before `execvp`, so a
//! program that is missing must be refused first, not by a failed `execvp`.

use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{RunError, guard};
use crate::env::Env;

/// What `execvp` searches when `PATH` is not set.
const DEFAULT_PATH: &str = "/usr/bin:/bin";

/// Replace this process with `argv`, in `cwd` and with exactly `env` when given;
/// the caller's directory and environment otherwise. Returns only when the
/// program could not start: [`RunError::EmptyArgv`] for an empty argv or program,
/// [`RunError::Spawn`] naming the program for anything else.
pub fn exec(argv: &[String], cwd: Option<&Path>, env: Option<&Env>) -> RunError {
  let Some(program) = argv.first().filter(|program| !program.is_empty()) else {
    return RunError::EmptyArgv;
  };
  let path = match env {
    Some(env) => env.get("PATH").map(str::to_owned),
    None => Env::process().get("PATH").map(str::to_owned),
  };
  let Some(resolved) = resolve(program, path.as_deref().unwrap_or(DEFAULT_PATH)) else {
    return RunError::Spawn(format!("{program}: not found on PATH"));
  };
  let mut command = Command::new(resolved);
  command.arg0(program).args(argv.iter().skip(1));
  if let Some(cwd) = cwd {
    command.current_dir(cwd);
  }
  if let Some(env) = env {
    command.env_clear().envs(env.vars());
  }
  let err = guard::unblocked(|| command.exec());
  RunError::Spawn(format!("{program}: {err}"))
}

/// `program` itself when it names a path, else the first executable file of
/// that name in the `:`-separated `path`, as `execvp` searches.
fn resolve(program: &str, path: &str) -> Option<PathBuf> {
  if program.contains('/') {
    return Some(PathBuf::from(program));
  }
  path
    .split(':')
    .map(|dir| Path::new(if dir.is_empty() { "." } else { dir }).join(program))
    .find(|candidate| executable(candidate))
}

fn executable(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

// These tests only reach the failure paths that return before std's `exec`
// changes this process: a program that starts would replace the test runner.
// The black-box tests of `toolu sandbox run` cover the success path.
#[cfg(test)]
#[path = "tests/exec_test.rs"]
mod tests;
