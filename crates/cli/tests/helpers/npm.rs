//! The shared harness for the npm wrapper (`toolu-plugins`) tests: the real
//! `node`, the built `toolu`, shell stand-ins, and one wrapper run.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The `toolu` binary under test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A helper's result; the tests unwrap it.
pub(crate) type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// A `PATH` with no `toolu` on it.
pub(crate) const SYSTEM: &str = "/usr/bin:/bin";

/// The workspace checkout.
pub(crate) fn repo() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The wrapper script.
pub(crate) fn wrapper() -> PathBuf {
  repo().join("tools/toolu-cli/npm/cli.js")
}

/// The first `node` on this process's `PATH`; these tests need it.
pub(crate) fn node() -> Res<PathBuf> {
  let path = std::env::var_os("PATH").unwrap_or_default();
  std::env::split_paths(&path)
    .map(|dir| dir.join("node"))
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| "node is not on PATH; the npm wrapper tests need it".into())
}

/// An executable `#!/bin/sh` script at `path`.
pub(crate) fn script(path: &Path, body: &str) -> Res<PathBuf> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(path, format!("#!/bin/sh\n{body}\n"))?;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  Ok(path.to_path_buf())
}

/// A stand-in native binary that answers the probes and logs every other call.
pub(crate) fn native_stub(path: &Path, log: &Path, tail: &str) -> Res<PathBuf> {
  script(
    path,
    &format!(
      "case \"$1\" in --hook-protocol) echo 2; exit 0 ;; esac\n\
       [ \"$3\" = --help ] && {{ echo \"Usage: toolu plugins install\"; exit 0; }}\n\
       echo \"$*\" >> '{}'\n{tail}",
      log.display()
    ),
  )
}

/// What one wrapper run printed and how it ended.
#[derive(Debug)]
pub(crate) struct Run {
  pub(crate) code: Option<i32>,
  pub(crate) stdout: String,
  pub(crate) stderr: String,
}

/// `node <cli> args` with exactly `env` (plus `TERM`, and under coverage the profile
/// path, so the instrumented `toolu` never writes a profile into `cwd`), in `cwd`.
pub(crate) fn run(cli: &Path, args: &[&str], env: &[(&str, &str)], cwd: &Path) -> Res<Run> {
  let mut command = Command::new(node()?);
  command
    .arg(cli)
    .args(args)
    .env_clear()
    .env("TERM", "dumb")
    .envs(env.iter().copied())
    .current_dir(cwd);
  if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let output = command.output()?;
  Ok(Run {
    code: output.status.code(),
    stdout: String::from_utf8(output.stdout)?,
    stderr: String::from_utf8(output.stderr)?,
  })
}

/// A canonicalized temporary directory.
pub(crate) fn temp() -> Res<(tempfile::TempDir, PathBuf)> {
  let dir = tempfile::tempdir()?;
  let root = std::fs::canonicalize(dir.path())?;
  Ok((dir, root))
}
