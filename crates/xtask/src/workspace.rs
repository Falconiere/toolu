//! The workspace a task judges: its members and the files git lists under it.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::metadata;

/// One workspace member, by its directory relative to the root.
#[derive(Debug)]
pub(crate) struct Member {
  /// The package name.
  pub(crate) name: String,
  /// The package directory relative to the root.
  pub(crate) dir: PathBuf,
  /// Whether the package has a library target.
  pub(crate) is_lib: bool,
}

/// The workspace at `root`.
#[derive(Debug)]
pub(crate) struct Workspace {
  /// The absolute root directory.
  pub(crate) root: PathBuf,
  /// Members, longest directory first so `member_of` finds the innermost.
  pub(crate) members: Vec<Member>,
  /// Tracked and untracked-but-not-ignored files, relative to the root.
  pub(crate) files: Vec<PathBuf>,
}

impl Workspace {
  /// Load the members and list the files of the workspace at `root`.
  pub(crate) fn load(root: &Path) -> Result<Self, String> {
    let metadata = metadata::load(root)?;
    let mut members: Vec<Member> = metadata
      .members()
      .map(|package| Member {
        name: package.name.clone(),
        dir: relative(root, package.dir()),
        is_lib: package.has_target("lib"),
      })
      .collect();
    members.sort_by_key(|member| std::cmp::Reverse(member.dir.components().count()));
    Ok(Workspace {
      root: root.to_path_buf(),
      members,
      files: list_files(root)?,
    })
  }

  /// The member whose directory holds `path`.
  pub(crate) fn member_of(&self, path: &Path) -> Option<&Member> {
    self
      .members
      .iter()
      .find(|member| path.starts_with(&member.dir))
  }

  /// Listed files under `dir` with extension `ext`.
  pub(crate) fn files_in<'a>(
    &'a self,
    dir: &'a str,
    ext: &'a str,
  ) -> impl Iterator<Item = &'a Path> {
    self
      .files
      .iter()
      .filter(move |file| file.starts_with(dir) && file.extension().is_some_and(|e| e == ext))
      .map(PathBuf::as_path)
  }

  /// The text of the listed file `rel`.
  pub(crate) fn read(&self, rel: &Path) -> Result<String, String> {
    std::fs::read_to_string(self.root.join(rel))
      .map_err(|err| format!("cannot read {}: {err}", rel.display()))
  }
}

/// The components of a relative path as strings.
pub(crate) fn parts(path: &Path) -> Vec<String> {
  path
    .iter()
    .map(|part| part.to_string_lossy().into_owned())
    .collect()
}

/// `path` relative to `root`, or `path` itself when it is outside.
pub(crate) fn relative(root: &Path, path: &Path) -> PathBuf {
  path
    .strip_prefix(root)
    .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}

/// `git ls-files --cached --others --exclude-standard`, existing files only, sorted.
pub(crate) fn list_files(root: &Path) -> Result<Vec<PathBuf>, String> {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args([
      "ls-files",
      "-z",
      "--cached",
      "--others",
      "--exclude-standard",
    ])
    .output()
    .map_err(|err| format!("cannot run git ls-files: {err}"))?;
  if !output.status.success() {
    return Err(format!(
      "git ls-files failed in {}: {}",
      root.display(),
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  let mut files: Vec<PathBuf> = output
    .stdout
    .split(|byte| *byte == 0)
    .filter(|name| !name.is_empty())
    .map(|name| PathBuf::from(String::from_utf8_lossy(name).into_owned()))
    .filter(|file| root.join(file).is_file())
    .collect();
  files.sort();
  files.dedup();
  Ok(files)
}

#[cfg(test)]
#[path = "tests/workspace_test.rs"]
mod tests;
