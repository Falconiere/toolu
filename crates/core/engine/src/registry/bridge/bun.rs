//! Finding Bun for the bridge, and the once-per-session note that it is missing.
//! Bun is looked for the way the generated launcher looks, in the hook's own
//! environment: `TOOLU_BUN`, `bun` on `PATH`, then `~/.bun/bin/bun`.

use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

/// Bun, as far as this hook call knows it.
#[derive(Debug, Default)]
pub(crate) struct BunState {
  /// Whether Bun was looked for yet.
  pub(crate) looked: bool,
  /// The executable, once looked for.
  pub(crate) found: Option<PathBuf>,
  /// Whether this call already added the missing-Bun advisory.
  pub(crate) advised: bool,
}

impl BunState {
  /// Bun for `env`, looked for once per hook call.
  pub(crate) fn bun(&mut self, env: &Env) -> Option<PathBuf> {
    if !self.looked {
      self.looked = true;
      self.found = find_bun(env);
    }
    self.found.clone()
  }
}

/// A regular file someone may execute.
fn executable(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// `bun` installed as a project's own tool. The post-tool `PATH` puts
/// `<project>/node_modules/.bin` first; that file is not the bridge.
fn project_bin(path: &Path) -> bool {
  let dir = path.parent();
  let modules = dir.and_then(Path::parent);
  dir.is_some_and(|dir| dir.file_name().is_some_and(|name| name == ".bin"))
    && modules.is_some_and(|dir| dir.file_name().is_some_and(|name| name == "node_modules"))
}

/// `TOOLU_BUN`, then `bun` on `PATH`, then `$HOME/.bun/bin/bun`.
/// A `node_modules/.bin/bun` on `PATH` is skipped.
pub(crate) fn find_bun(env: &Env) -> Option<PathBuf> {
  let explicit = env.get("TOOLU_BUN").map(PathBuf::from);
  let on_path = env.get("PATH").and_then(|path| {
    path
      .split(':')
      .filter(|dir| !dir.is_empty())
      .map(|dir| Path::new(dir).join("bun"))
      .find(|candidate| executable(candidate) && !project_bin(candidate))
  });
  let home = env.home().join(".bun/bin/bun");
  [explicit, on_path, Some(home)]
    .into_iter()
    .flatten()
    .find(|candidate| executable(candidate))
}

/// A session id as a file name: `[A-Za-z0-9._-]` kept, anything else `_`, at
/// most 128 characters, `unknown` when empty.
pub(crate) fn session_file(session_id: &str) -> String {
  let name: String = session_id
    .chars()
    .take(128)
    .map(|c| {
      if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
        c
      } else {
        '_'
      }
    })
    .collect();
  if name.is_empty() || name == "." || name == ".." {
    "unknown".to_owned()
  } else {
    name
  }
}

/// Whether to show the missing-Bun advisory: true once per session, claimed by
/// creating `<dir>/<session>`; a marker that cannot be written shows it again.
pub(crate) fn claim_advisory(dir: Option<&Path>, session_id: &str) -> bool {
  let Some(dir) = dir else {
    return true;
  };
  if std::fs::create_dir_all(dir).is_err() {
    return true;
  }
  let marker = dir.join(session_file(session_id));
  match std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(marker)
  {
    Ok(_) => true,
    Err(err) => err.kind() != ErrorKind::AlreadyExists,
  }
}

#[cfg(test)]
#[path = "tests/bun_test.rs"]
mod tests;
