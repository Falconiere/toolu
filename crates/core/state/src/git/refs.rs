//! Refs read from the files backend: `HEAD`, loose refs and `packed-refs`.
//! Each answer is `None` where only git can say it exactly (a reftable
//! repository, a symlinked or unusual `HEAD`, a short name git would lengthen).

use std::collections::BTreeSet;
use std::path::Path;

use toolu_runtime::git::Repo;

/// `git rev-parse --abbrev-ref HEAD`: the branch, `"HEAD"` when detached or
/// unborn, or `None` to ask git.
pub(crate) fn abbrev_head(repo: &Repo) -> Option<String> {
  if repo.common_dir.join("reftable").exists() {
    return None;
  }
  let head = repo.git_dir.join("HEAD");
  if std::fs::symlink_metadata(&head)
    .ok()?
    .file_type()
    .is_symlink()
  {
    return None;
  }
  let text = std::fs::read_to_string(&head).ok()?;
  let Some(target) = text.strip_prefix("ref:") else {
    return is_object_id(text.trim_end()).then(|| "HEAD".to_owned());
  };
  let target = target.trim();
  let name = target.strip_prefix("refs/heads/")?;
  let packed = packed_refs(&repo.common_dir);
  if !ref_exists(repo, target, &packed) {
    return Some("HEAD".to_owned());
  }
  let lengthened = [
    format!("refs/{name}"),
    format!("refs/tags/{name}"),
    format!("refs/remotes/{name}"),
    format!("refs/remotes/{name}/HEAD"),
  ];
  let ambiguous = repo.git_dir.join(name).is_file()
    || repo.common_dir.join(name).is_file()
    || lengthened
      .iter()
      .any(|other| ref_exists(repo, other, &packed));
  (!ambiguous).then(|| name.to_owned())
}

/// The target of the symbolic ref `refname` (`git symbolic-ref`), `Some("")`
/// when it is absent or not symbolic, `None` to ask git (reftable, or a
/// symlinked ref, which `core.preferSymlinkRefs` writes).
pub(crate) fn symbolic_target(repo: &Repo, refname: &str) -> Option<String> {
  if repo.common_dir.join("reftable").exists() {
    return None;
  }
  let path = repo.common_dir.join(refname);
  if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
    return None;
  }
  let Ok(text) = std::fs::read_to_string(path) else {
    return Some(String::new());
  };
  Some(
    text
      .strip_prefix("ref:")
      .map(|target| target.trim().to_owned())
      .unwrap_or_default(),
  )
}

/// A SHA-1 or SHA-256 object id in hex.
fn is_object_id(text: &str) -> bool {
  matches!(text.len(), 40 | 64) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A loose ref file (per-worktree or shared) or a `packed-refs` line.
fn ref_exists(repo: &Repo, refname: &str, packed: &BTreeSet<String>) -> bool {
  repo.git_dir.join(refname).is_file()
    || repo.common_dir.join(refname).is_file()
    || packed.contains(refname)
}

/// The ref names `packed-refs` lists; peeled (`^`) and comment lines skipped.
fn packed_refs(common_dir: &Path) -> BTreeSet<String> {
  let Ok(text) = std::fs::read_to_string(common_dir.join("packed-refs")) else {
    return BTreeSet::new();
  };
  text
    .lines()
    .filter(|line| !line.starts_with('#') && !line.starts_with('^'))
    .filter_map(|line| {
      line
        .split_once(' ')
        .map(|(_, name)| name.trim_end().to_owned())
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/refs_test.rs"]
mod tests;
