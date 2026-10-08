//! A private-index checkpoint of one dirty worktree, matching the TypeScript
//! sequence: `read-tree`, `add -A`, `write-tree`, `commit-tree`, `update-ref`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

/// Snapshot `worktree` at `refs/epic-wip/<key>`. A clean tree is left alone.
///
/// # Errors
/// Git is missing or a command fails.
pub(crate) fn snapshot(worktree: &str, key: &str, trace: Option<&Path>) -> Result<(), String> {
  let cwd = Path::new(worktree);
  if !cwd.join(".git").exists() {
    return Ok(());
  }
  let head = git(cwd, &["rev-parse", "HEAD"], None, trace)?;
  let status = git(cwd, &["status", "--porcelain"], None, trace)?;
  if status.is_empty() {
    return Ok(());
  }
  let tree = write_tree(cwd, trace)?;
  let sha = commit(cwd, key, head.trim(), tree.trim(), trace)?;
  let reff = format!("refs/epic-wip/{key}");
  git(
    cwd,
    &[
      "update-ref",
      "--create-reflog",
      "-m",
      "epic checkpoint",
      &reff,
      sha.trim(),
    ],
    None,
    trace,
  )?;
  Ok(())
}

fn write_tree(cwd: &Path, trace: Option<&Path>) -> Result<String, String> {
  let common = git(
    cwd,
    &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    None,
    trace,
  )?;
  let index = PathBuf::from(common.trim())
    .join("toolu")
    .join("checkpoint")
    .join(short_id(cwd));
  let index = index.join("index");
  if let Some(parent) = index.parent() {
    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
  }
  git(cwd, &["read-tree", "HEAD"], Some(&index), trace)?;
  git(cwd, &["add", "-A"], Some(&index), trace)?;
  git(cwd, &["write-tree"], Some(&index), trace)
}

fn commit(
  cwd: &Path,
  key: &str,
  head: &str,
  tree: &str,
  trace: Option<&Path>,
) -> Result<String, String> {
  let message = format!("epic-wip {key}");
  git(
    cwd,
    &["commit-tree", tree, "-p", head, "-m", &message],
    None,
    trace,
  )
}

fn git(
  cwd: &Path,
  args: &[&str],
  index: Option<&Path>,
  trace: Option<&Path>,
) -> Result<String, String> {
  let mut line = vec!["git".to_owned(), "-C".to_owned(), cwd.display().to_string()];
  line.extend(args.iter().map(|arg| (*arg).to_owned()));
  let mut spec = Spec::new(line);
  spec.timeout = Duration::from_secs(30);
  spec.env = Some(git_env(index, trace));
  let out = run(&spec).map_err(|err| format!("{err:?}"))?;
  if out.exit_code != 0 {
    let name = args.first().copied().unwrap_or("git");
    return Err(format!("git {name}: {}", out.stderr.trim()));
  }
  Ok(out.stdout.trim().to_owned())
}

fn git_env(index: Option<&Path>, trace: Option<&Path>) -> Env {
  let mut env = Env::process()
    .with("GIT_AUTHOR_NAME", "epic-orchestrator")
    .with("GIT_AUTHOR_EMAIL", "epic-orchestrator@localhost")
    .with("GIT_COMMITTER_NAME", "epic-orchestrator")
    .with("GIT_COMMITTER_EMAIL", "epic-orchestrator@localhost");
  if let Some(index) = index {
    let text = index.display().to_string();
    env = env.with("GIT_INDEX_FILE", &text);
  }
  if let Some(trace) = trace {
    let text = trace.display().to_string();
    env = env.with("GIT_TRACE2_EVENT", &text);
  }
  env
}

fn short_id(path: &Path) -> String {
  let mut hash = 0xcbf2_9ce4_8422_2325u64;
  for byte in path.display().to_string().into_bytes() {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(0x100_0000_01b3);
  }
  format!("{hash:016x}")
}

#[cfg(test)]
#[path = "tests/checkpoint_test.rs"]
mod tests;
