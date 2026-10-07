//! The engine's pid lock. A live holder is left alone; a dead one is reclaimed.
//! The journal uses the same exclusive-create file for its short appends.

use std::path::{Path, PathBuf};

use nix::errno::Errno;
use nix::sys::signal::kill;
use nix::unistd::Pid;

/// A lock file this process created.
#[derive(Debug)]
pub(crate) struct Held {
  path: PathBuf,
  body: String,
}

impl Held {
  /// Create `path` for this process.
  ///
  /// # Errors
  /// A live process holds it (`engine-busy` when `busy` is that label), or the
  /// file cannot be created.
  pub(crate) fn acquire(path: &Path, busy: &str) -> Result<Self, String> {
    let body = format!("{}\n", std::process::id());
    for _ in 0..3 {
      match create_new(path, &body) {
        Ok(()) => {
          return Ok(Self {
            path: path.to_path_buf(),
            body,
          });
        }
        Err(Take::Busy) if reclaim_if_dead(path) => {}
        Err(Take::Busy) => return Err(busy.to_owned()),
        Err(Take::Io(err)) => return Err(err),
      }
    }
    Err(busy.to_owned())
  }
}

impl Drop for Held {
  fn drop(&mut self) {
    if std::fs::read_to_string(&self.path).ok().as_deref() == Some(self.body.as_str()) {
      let _gone = std::fs::remove_file(&self.path);
    }
  }
}

enum Take {
  Busy,
  Io(String),
}

fn create_new(path: &Path, body: &str) -> Result<(), Take> {
  if let Some(parent) = path.parent()
    && std::fs::create_dir_all(parent).is_err()
  {
    return Err(Take::Io(format!("could not create {}", parent.display())));
  }
  let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
    return Err(Take::Io(format!("could not create {}", path.display())));
  };
  let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
  write_tmp(&tmp, body)?;
  let linked = std::fs::hard_link(&tmp, path);
  let _removed = std::fs::remove_file(&tmp);
  match linked {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Err(Take::Busy),
    Err(err) => Err(Take::Io(err.to_string())),
  }
}

fn write_tmp(tmp: &Path, body: &str) -> Result<(), Take> {
  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .open(tmp)
    .map_err(|err| Take::Io(err.to_string()))?;
  std::io::Write::write_all(&mut file, body.as_bytes()).map_err(|err| Take::Io(err.to_string()))
}

pub(crate) fn live(path: &Path) -> bool {
  let Ok(text) = std::fs::read_to_string(path) else {
    return false;
  };
  pid_of(&text).is_some_and(process_alive)
}

fn reclaim_if_dead(path: &Path) -> bool {
  let Ok(text) = std::fs::read_to_string(path) else {
    return false;
  };
  let Some(pid) = pid_of(&text) else {
    return false;
  };
  if pid > 0 && live(path) {
    return false;
  }
  if std::fs::read_to_string(path).ok().as_deref() == Some(text.as_str()) {
    std::fs::remove_file(path).is_ok()
  } else {
    false
  }
}

fn pid_of(text: &str) -> Option<i32> {
  text.split_whitespace().next()?.parse().ok()
}

fn process_alive(pid: i32) -> bool {
  pid > 0 && !matches!(kill(Pid::from_raw(pid), None), Err(Errno::ESRCH))
}

#[cfg(test)]
#[path = "tests/lock_test.rs"]
mod tests;
