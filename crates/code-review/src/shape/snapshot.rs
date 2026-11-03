//! Capture local review evidence without changing HEAD or the user's index.

use std::path::Path;
use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;

use super::git_io::{git, git_process_input, text};
use super::parse::{Status, names, statuses};
use super::{ShapeError, Shaped, dirty, populate, resolve_base};
use crate::contract::Coverage;
use toolu_state::diff_sha::diff_sha;

/// Shape the branch and local changes from one immutable tree.
pub(crate) fn review(
  env: &Env,
  root: &Path,
  base: Option<&str>,
  state_root: &Path,
) -> Result<Shaped, ShapeError> {
  let top =
    toplevel(env, root).ok_or_else(|| ShapeError::Git("not inside a Git work tree".to_owned()))?;
  if !dirty::dirty_except_state(env, &top, state_root)? {
    let mut shaped = super::shape(env, &top, base)?;
    if !dirty::dirty_except_state(env, &top, state_root)? {
      shaped.uncommitted = false;
      return Ok(shaped);
    }
  }
  let (excludes, _) = dirty::exclusions(env, &top, state_root)?;
  let tree = capture(env, &top, &excludes)?;
  let (mut shaped, statuses) = start(env, &top, base, Some(tree))?;
  shaped.uncommitted = true;
  populate(env, &top, statuses, &mut shaped)?;
  Ok(shaped)
}

fn capture(env: &Env, root: &Path, excludes: &[String]) -> Result<String, ShapeError> {
  let temp = tempfile::tempdir().map_err(|err| ShapeError::Git(err.to_string()))?;
  let path = temp.path().join("index");
  let original = text(git(
    env,
    root,
    &["rev-parse", "--path-format=absolute", "--git-path", "index"],
  )?)?;
  let snapshot = env.clone().with("GIT_INDEX_FILE", &path.to_string_lossy());
  match copy_index(original.trim(), &path) {
    Ok(()) => (),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      git(&snapshot, root, &["read-tree", "HEAD"])?;
    }
    Err(err) => return Err(ShapeError::Git(format!("copy index: {err}"))),
  }
  if !git(&snapshot, root, &["ls-files", "--unmerged", "-z"])?.is_empty() {
    return Err(ShapeError::Git(
      "resolve merge conflicts before reviewing".to_owned(),
    ));
  }
  let mut args = vec!["add", "-A", "--", ":/"];
  args.extend(excludes.iter().map(String::as_str));
  git(&snapshot, root, &args)?;
  // A copied index may already contain staged plugin state; restore only those entries.
  if !excludes.is_empty() {
    let paths: Vec<String> = excludes
      .iter()
      .map(|path| path.replacen(":(exclude,literal)", ":(literal)", 1))
      .collect();
    let mut args = vec!["reset", "-q", "HEAD", "--"];
    args.extend(paths.iter().map(String::as_str));
    git(&snapshot, root, &args)?;
  }
  text(git(&snapshot, root, &["write-tree"])?).map(|tree| tree.trim().to_owned())
}

/// Identify local evidence separately from the committed push-gate hash.
pub(super) fn hash(
  env: &Env,
  root: &Path,
  merge_base: &str,
  source_ref: &str,
) -> Result<String, ShapeError> {
  let input = format!("local review\n{merge_base}\n{source_ref}\n").into_bytes();
  let out = git_process_input(env, root, &["hash-object", "--stdin"], 256, input)?;
  if out.exit_code != 0 || out.timed_out || out.truncated {
    return Err(ShapeError::Git("local diff hash failed".to_owned()));
  }
  text(out.stdout_bytes).map(|hash| hash.trim().to_owned())
}

fn copy_index(original: &str, copy: &Path) -> std::io::Result<()> {
  // Git uses index mtime to detect racy-clean entries after same-size edits.
  let modified = std::fs::metadata(original)?.modified()?;
  std::fs::copy(original, copy)?;
  std::fs::File::open(copy)?.set_modified(modified)
}

/// Resolve the base and metadata for a pinned committed or local tree.
pub(super) fn start(
  env: &Env,
  root: &Path,
  requested_base: Option<&str>,
  snapshot: Option<String>,
) -> Result<(Shaped, Vec<Status>), ShapeError> {
  let (base_ref, merge_base) = resolve_base(env, root, requested_base)?;
  let committed_hash = if snapshot.is_none() {
    Some(
      diff_sha(env, root, &base_ref)
        .ok_or_else(|| ShapeError::Git("gate diff hash failed".to_owned()))?,
    )
  } else {
    None
  };
  let source_ref = match snapshot {
    Some(tree) => tree,
    None => text(git(env, root, &["rev-parse", "HEAD"])?)?
      .trim()
      .to_owned(),
  };
  let diff_sha = match committed_hash {
    Some(hash) => hash,
    None => hash(env, root, &merge_base, &source_ref)?,
  };
  let range = format!("{merge_base}..{source_ref}");
  let gate_files = names(&git(env, root, &["diff", &range, "--name-only", "-z"])?)?;
  if gate_files.is_empty() {
    return Err(ShapeError::EmptyDiff);
  }
  let metadata = git(env, root, &["diff", "-M", "--name-status", "-z", &range])?;
  let statuses = statuses(&metadata)?;
  let uncommitted = dirty::dirty_with_excludes(env, root, &[])?;
  let shaped = Shaped {
    base_ref,
    merge_base,
    source_ref,
    diff_sha,
    gate_files,
    files: Vec::new(),
    chunks: Vec::new(),
    coverage: Coverage::default(),
    renames: Vec::new(),
    uncommitted,
  };
  Ok((shaped, statuses))
}

#[cfg(test)]
#[path = "tests/snapshot_test.rs"]
mod tests;
