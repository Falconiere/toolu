//! Tool availability (`detect-tools.ts`): bash `command -v NAME` for an external tool,
//! and `detect_ast_grep`. A `PATH` scan with no subprocess, cached per `PATH` and name
//! for the life of the process; a changed `PATH` is a different key. Shell builtins,
//! functions and aliases are not tools and are never reported.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use nix::unistd::{AccessFlags, access};
use toolu_runtime::env::Env;

/// Answers by `(PATH, name)`.
type Cache = Mutex<HashMap<(String, String), bool>>;

/// The process-lifetime answers of absolute `PATH`s.
static CACHE: OnceLock<Cache> = OnceLock::new();

/// What `command -v` accepts from `PATH`: an existing entry that is not a directory
/// (symlinks followed). Bash prefers an executable one, but when none exists it still
/// reports the first non-executable file and exits 0, so the execute bit does not
/// change the answer.
fn is_command_file(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| !meta.is_dir())
}

/// A path given with a `/` must be an executable non-directory: bash has no fallback there.
fn is_executable(path: &Path) -> bool {
  access(path, AccessFlags::X_OK).is_ok() && is_command_file(path)
}

/// Whether any of `dirs` holds a command file `name`. An empty entry is the current
/// directory, as in bash.
fn scan(name: &str, dirs: &[&str]) -> bool {
  dirs.iter().any(|dir| {
    let dir = if dir.is_empty() { "." } else { dir };
    is_command_file(&Path::new(dir).join(name))
  })
}

/// The cache, recovered when a holder panicked: its entries are plain answers.
fn locked(cache: &Cache) -> MutexGuard<'_, HashMap<(String, String), bool>> {
  cache.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The cached answer for `(path, name)`, probing and remembering it when absent.
fn cached(path: &str, name: &str, probe: impl FnOnce() -> bool) -> bool {
  let key = (path.to_owned(), name.to_owned());
  let cache = CACHE.get_or_init(Mutex::default);
  if let Some(hit) = locked(cache).get(&key) {
    return *hit;
  }
  let found = probe();
  locked(cache).insert(key, found);
  found
}

/// Whether `command -v name` finds a command file. A name holding `/` is checked as a path.
pub fn tool_available(name: &str, env: &Env) -> bool {
  if name.is_empty() {
    return false;
  }
  if name.contains('/') {
    return is_executable(Path::new(name));
  }
  let path = env.get("PATH").unwrap_or("");
  let dirs: Vec<&str> = path.split(':').collect();
  // Entries relative to the cwd make the answer cwd-dependent: probe, never cache.
  if dirs.iter().any(|dir| !dir.starts_with('/')) {
    return scan(name, &dirs);
  }
  cached(path, name, || scan(name, &dirs))
}

/// `detect_ast_grep`: `sg` or `ast-grep` is on `PATH`.
pub fn detect_ast_grep(env: &Env) -> bool {
  tool_available("sg", env) || tool_available("ast-grep", env)
}

#[cfg(test)]
#[path = "tests/tools_test.rs"]
mod tests;
