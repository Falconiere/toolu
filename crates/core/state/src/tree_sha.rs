//! The working-tree content hash: `git write-tree` over a temporary index that
//! stages tracked and untracked-but-not-ignored files. The real index and the
//! working-tree files are never written. Returns `None` when git fails or the
//! path is not a repository, as `diff_sha` does.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Output, Spec, run};

fn git(env: &Env, root: &Path, args: &[&str]) -> Option<Output> {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(root.display().to_string());
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  spec.max_output_bytes = usize::MAX;
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.truncated)
}

/// The working-tree content hash, or `None` when git fails or the path is not a
/// repository.
pub fn tree_sha(env: &Env, repo_root: &Path) -> Option<String> {
  // A missing path, not an empty file: an empty NamedTempFile is not a git index.
  let scratch = tempfile::tempdir().ok()?;
  let index_path = scratch.path().join("index");
  let index_env = env
    .clone()
    .with("GIT_INDEX_FILE", &index_path.display().to_string());
  git(&index_env, repo_root, &["read-tree", "HEAD"])?;
  git(&index_env, repo_root, &["add", "-A"])?;
  let tree = git(&index_env, repo_root, &["write-tree"])?;
  let sha = tree.stdout.trim();
  (!sha.is_empty()).then(|| sha.to_owned())
}

#[cfg(test)]
#[path = "tests/tree_sha_test.rs"]
mod tests;
