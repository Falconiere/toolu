//! `git` for the sandbox's tests: real repositories with a fixed identity and no
//! system config, so a developer's git setup never changes a result.

use std::io;
use std::path::Path;
use std::process::Command;

/// Run `git <args>` in `dir`; an exit other than 0 is an error naming the args.
pub(crate) fn git(dir: &Path, args: &[&str]) -> io::Result<()> {
  let status = Command::new("git")
    .args(args)
    .current_dir(dir)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@t")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@t")
    .status()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::other(format!("git {args:?} failed")))
  }
}
