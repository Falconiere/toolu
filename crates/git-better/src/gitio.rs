//! Running git for this plugin: one bounded process per call, through the
//! runtime's process runner, in the repository the command worked in.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

/// The changes of one file in one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileChange {
  /// The path after the commit.
  pub(crate) path: String,
  /// The path before a rename or copy.
  pub(crate) old_path: Option<String>,
  /// Git's status letter: `A`, `M`, `D`, `R`, `C`, `T`.
  pub(crate) status: String,
  /// Lines added; `None` for a binary file.
  pub(crate) added: Option<u64>,
  /// Lines deleted; `None` for a binary file.
  pub(crate) deleted: Option<u64>,
}

/// `git -C <dir> <args>` with `stdin`; stdout when it exits 0, else why not.
pub(crate) fn git(env: &Env, dir: &Path, args: &[&str], stdin: &[u8]) -> Result<String, String> {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(dir.display().to_string());
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  spec.stdin = stdin.to_vec();
  let out = run(&spec).map_err(|error| format!("git {}: {error:?}", args.join(" ")))?;
  if out.exit_code != 0 || out.timed_out {
    let why = out.stderr.trim();
    return Err(format!("git {} failed: {why}", args.join(" ")));
  }
  Ok(out.stdout)
}

fn count(text: &str) -> Option<u64> {
  text.parse().ok()
}

/// `diff-tree -z --numstat` and `--name-status` of the same diff, joined by path.
fn joined(numstat: &str, status: &str) -> Vec<FileChange> {
  let mut statuses = status.split('\0').filter(|field| !field.is_empty());
  let mut lines = numstat.split('\0');
  let mut found = Vec::new();
  while let Some(letter) = statuses.next() {
    let renamed = letter.starts_with(['R', 'C']);
    let first = statuses.next().unwrap_or_default().to_owned();
    let (old_path, path) = if renamed {
      (Some(first), statuses.next().unwrap_or_default().to_owned())
    } else {
      (None, first)
    };
    let counts = lines.next().unwrap_or_default();
    if renamed {
      lines.next();
      lines.next();
    }
    let mut parts = counts.splitn(3, '\t');
    let (added, deleted) = (parts.next().and_then(count), parts.next().and_then(count));
    found.push(FileChange {
      path,
      old_path,
      status: letter.chars().take(1).collect(),
      added,
      deleted,
    });
  }
  found
}

fn diff(env: &Env, dir: &Path, form: &[&str], what: &str) -> Result<String, String> {
  let mut args = vec!["diff-tree", "-r", "-M", "-z", "--no-commit-id", what];
  args.extend(form);
  git(env, dir, &args, &[])
}

/// Every file `commit` changed against its first parent (or the empty tree for a
/// root commit), renames detected.
pub(crate) fn changes(env: &Env, dir: &Path, commit: &str) -> Result<Vec<FileChange>, String> {
  let parent = format!("{commit}^");
  let two = [parent.as_str(), commit];
  let (numstat, status) = match diff(env, dir, &two, "--numstat") {
    Ok(numstat) => (numstat, diff(env, dir, &two, "--name-status")?),
    Err(_) => (
      diff(env, dir, &["--root", commit], "--numstat")?,
      diff(env, dir, &["--root", commit], "--name-status")?,
    ),
  };
  Ok(joined(&numstat, &status))
}

#[cfg(test)]
#[path = "tests/gitio_test.rs"]
mod tests;
