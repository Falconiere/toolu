//! The retained private index of one worktree's checkpoints: where it lives
//! in the shared git directory, the HEAD it was read from, and the rebuild of
//! a missing, stale, locked or corrupt index.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_GIT_OUTPUT_BYTES: usize = 64 * 1024;

/// The retained index's record: the worktree and HEAD it was read from.
#[derive(Debug, Serialize, Deserialize)]
struct Meta {
  version: u8,
  worktree: String,
  head: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  index: Option<String>,
}

/// Where one worktree's retained index lives.
pub(super) struct Storage {
  /// The retained index directory.
  pub path: PathBuf,
  /// The canonical worktree.
  pub worktree: PathBuf,
}

/// git in one worktree, tracing to `trace`.
pub(super) struct Git<'a> {
  /// The canonical worktree.
  pub worktree: PathBuf,
  /// The `GIT_TRACE2_EVENT` file.
  pub trace: Option<&'a Path>,
}

impl Git<'_> {
  /// Trimmed stdout, or `None` when git failed, timed out or said too much.
  pub(super) fn run(&self, args: &[&str], index: Option<&Path>) -> Option<String> {
    let mut line = vec![
      "git".to_owned(),
      "-C".to_owned(),
      self.worktree.display().to_string(),
    ];
    line.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut spec = Spec::new(line);
    spec.timeout = COMMAND_TIMEOUT;
    spec.max_output_bytes = MAX_GIT_OUTPUT_BYTES;
    spec.env = Some(git_env(index, self.trace));
    let out = run(&spec).ok()?;
    (out.exit_code == 0 && !out.timed_out && !out.truncated).then(|| out.stdout.trim().to_owned())
  }
}

fn git_env(index: Option<&Path>, trace: Option<&Path>) -> Env {
  let mut env = Env::process()
    .with("GIT_AUTHOR_NAME", "epic-orchestrator")
    .with("GIT_AUTHOR_EMAIL", "epic-orchestrator@localhost")
    .with("GIT_COMMITTER_NAME", "epic-orchestrator")
    .with("GIT_COMMITTER_EMAIL", "epic-orchestrator@localhost");
  if let Some(index) = index {
    env = env.with("GIT_INDEX_FILE", &index.display().to_string());
  }
  if let Some(trace) = trace {
    env = env.with("GIT_TRACE2_EVENT", &trace.display().to_string());
  }
  env
}

/// The retained index directory of `worktree` under its common git directory.
pub(super) fn storage(worktree: &Path, trace: Option<&Path>) -> Option<Storage> {
  let canonical = std::fs::canonicalize(worktree).ok()?;
  let git = Git {
    worktree: canonical.clone(),
    trace,
  };
  let common = git
    .run(
      &["rev-parse", "--path-format=absolute", "--git-common-dir"],
      None,
    )
    .filter(|common| !common.is_empty())?;
  let common = canonical.join(common);
  Some(Storage {
    path: common
      .join("toolu")
      .join("checkpoint")
      .join(short_id(&canonical)),
    worktree: canonical,
  })
}

/// `add -A` into the retained index (rebuilt from HEAD when it belongs to
/// another HEAD, is missing, locked or corrupt) and its tree.
pub(super) fn prepare_tree(git: &Git<'_>, storage: &Storage, head: &str) -> Option<String> {
  let meta_path = storage.path.join("meta.json");
  let meta: Option<Meta> = std::fs::read_to_string(&meta_path)
    .ok()
    .and_then(|text| serde_json::from_str(&text).ok());
  let mut index = storage.path.join(saved_index(meta.as_ref()));
  let worktree = storage.worktree.display().to_string();
  let reusable = meta.as_ref().is_some_and(|meta| {
    matches!(meta.version, 1 | 2) && meta.worktree == worktree && meta.head == head
  }) && index.exists()
    && !locked(&index);
  if !reusable {
    index = fresh_index(storage, &index);
    git.run(&["read-tree", "HEAD"], Some(&index))?;
    write_meta(storage, head, &index)?;
  }
  let mut staged = git.run(&["add", "-A"], Some(&index));
  if staged.is_none() && reusable {
    index = fresh_index(storage, &index);
    if git.run(&["read-tree", "HEAD"], Some(&index)).is_some() {
      write_meta(storage, head, &index)?;
      staged = git.run(&["add", "-A"], Some(&index));
    }
  }
  staged?;
  git.run(&["write-tree"], Some(&index))
}

fn saved_index(meta: Option<&Meta>) -> String {
  let valid = regex::Regex::new(r"^index(?:-[0-9a-f-]{36})?$");
  match meta {
    Some(Meta {
      version: 2,
      index: Some(index),
      ..
    }) if valid.is_ok_and(|re| re.is_match(index)) => index.clone(),
    Some(_) | None => "index".to_owned(),
  }
}

fn locked(index: &Path) -> bool {
  let mut lock = index.as_os_str().to_owned();
  lock.push(".lock");
  Path::new(&lock).exists()
}

/// A locked index is left to its writer and a new generation starts; any
/// other is removed and rebuilt in place.
fn fresh_index(storage: &Storage, index: &Path) -> PathBuf {
  if locked(index) {
    return storage
      .path
      .join(format!("index-{}", toolu_state::lock::token()));
  }
  let _removed = std::fs::remove_file(index);
  index.to_path_buf()
}

fn write_meta(storage: &Storage, head: &str, index: &Path) -> Option<()> {
  let meta = Meta {
    version: 2,
    worktree: storage.worktree.display().to_string(),
    head: head.to_owned(),
    index: index
      .file_name()
      .map(|name| name.to_string_lossy().into_owned()),
  };
  let text = serde_json::to_string_pretty(&meta).ok()?;
  toolu_runtime::atomic::write_atomic(&storage.path.join("meta.json"), &format!("{text}\n"))
    .then_some(())
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
#[path = "tests/index_test.rs"]
mod tests;
