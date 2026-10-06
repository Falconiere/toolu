//! The content-addressed branch-diff hash (`diff-sha.ts`): the git blob id of
//! `git diff --no-color <base>...HEAD`. It survives amend and rebase because it
//! hashes content, not commit ids; an empty diff is the empty-blob id. The diff
//! goes to `git hash-object` as the bytes git wrote, with no output budget, so
//! a large or non-UTF-8 diff hashes exactly. Each step runs under the default
//! deadline of `toolu_runtime::process` (30 s); past it there is no hash, which
//! TypeScript (no deadline) would still have computed.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Output, Spec, run};

fn git(env: &Env, root: &Path, args: &[&str], stdin: Vec<u8>) -> Option<Output> {
  let mut spec = Spec::new(["git", "-C"]);
  spec.argv.push(root.display().to_string());
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.env = Some(env.clone());
  spec.stdin = stdin;
  spec.max_output_bytes = usize::MAX;
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.truncated)
}

/// The hash, or `None` when either git step fails or prints nothing.
pub fn diff_sha(env: &Env, repo_root: &Path, base_ref: &str) -> Option<String> {
  // A ref starting with `-` would reach git as an option (`--output=...`), never a revision.
  if base_ref.starts_with('-') {
    return None;
  }
  let range = format!("{base_ref}...HEAD");
  let diff = git(env, repo_root, &["diff", "--no-color", &range], Vec::new())?;
  // Hashed in the repository, so its object format (SHA-1 or SHA-256) decides the id.
  let hash = git(
    env,
    repo_root,
    &["hash-object", "--stdin"],
    diff.stdout_bytes,
  )?;
  let sha = hash.stdout.trim();
  (!sha.is_empty()).then(|| sha.to_owned())
}

#[cfg(test)]
#[path = "tests/diff_sha_test.rs"]
mod tests;
