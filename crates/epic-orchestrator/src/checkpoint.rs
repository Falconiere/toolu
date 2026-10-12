//! Snapshot a worker's worktree so no progress is lost to a crash, a usage
//! limit, a host switch or a teardown (`scripts/checkpoint.ts`).
//!
//! The snapshot is a commit of the full working tree (tracked, staged and
//! untracked-but-not-ignored files) on top of HEAD, built with a retained
//! private index so the worker's own index and files are never touched. It is
//! stored at `refs/epic-wip/<key>` in the shared repository, with a reflog, so
//! it outlives the worktree. Restore with
//! `git checkout -b recover refs/epic-wip/<key>`.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use toolu_engine::resources::lock::acquire_lock;

mod index;

use index::{Git, Storage, prepare_tree, storage};

const LOCK_TIMEOUT: Duration = Duration::from_secs(1);

/// One snapshot's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Snapshot {
  /// The issue key.
  pub key: String,
  /// `refs/epic-wip/<key>`.
  #[serde(rename = "ref")]
  pub reference: String,
  /// The commit the ref names, when there is one.
  pub sha: Option<String>,
  /// The worktree had uncommitted work.
  pub dirty: bool,
  /// Commits on HEAD that no `origin` branch has.
  pub unpushed: u64,
  /// The ref moved.
  pub changed: bool,
  /// Why nothing was written.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub skipped: Option<String>,
}

impl Snapshot {
  fn base(key: &str) -> Self {
    Self {
      key: key.to_owned(),
      reference: format!("refs/epic-wip/{key}"),
      sha: None,
      dirty: false,
      unpushed: 0,
      changed: false,
      skipped: None,
    }
  }

  fn skip(&self, reason: &str) -> Self {
    Self {
      skipped: Some(reason.to_owned()),
      ..self.clone()
    }
  }

  /// Whether git failed, rather than there being nothing to do or another
  /// snapshot of the same worktree running.
  pub(crate) fn failed(&self) -> bool {
    self.skipped.as_deref().is_some_and(|reason| {
      !matches!(
        reason,
        "no worktree" | "nothing at risk" | "checkpoint busy"
      )
    })
  }
}

/// What one attempt ends with: a result, or a HEAD that moved under it.
enum Step {
  Done(Snapshot),
  Retry(Snapshot),
}

/// Create or refresh `refs/epic-wip/<key>` without touching the worker's index.
/// `trace` receives git's `GIT_TRACE2_EVENT` log.
pub(crate) fn snapshot(worktree: &str, key: &str, trace: Option<&Path>) -> Snapshot {
  let base = Snapshot::base(key);
  if worktree.is_empty() || !Path::new(worktree).join(".git").exists() {
    return base.skip("no worktree");
  }
  let Some(storage) = storage(Path::new(worktree), trace) else {
    return base.skip("no git directory");
  };
  if std::fs::create_dir_all(&storage.path).is_err() {
    return base.skip("checkpoint directory failed");
  }
  let mut lock_path = storage.path.clone().into_os_string();
  lock_path.push(".lock");
  let lock = match acquire_lock(Path::new(&lock_path), LOCK_TIMEOUT) {
    Ok(Some(lock)) => lock,
    Ok(None) => return base.skip("checkpoint busy"),
    // Not a busy peer: the snapshot did not happen, so the caller must see it fail.
    Err(_) => return base.skip("checkpoint lock failed"),
  };
  let git = Git {
    worktree: storage.worktree.clone(),
    trace,
  };
  let mut outcome = base.skip("HEAD changed during checkpoint");
  for attempt in 0..2 {
    match attempt_once(&git, &storage, &base) {
      Step::Done(done) => {
        outcome = done;
        break;
      }
      Step::Retry(moved) if attempt > 0 => outcome = moved,
      Step::Retry(_) => {}
    }
  }
  lock.release();
  outcome
}

fn attempt_once(git: &Git<'_>, storage: &Storage, base: &Snapshot) -> Step {
  let Some(head) = git.run(&["rev-parse", "HEAD"], None) else {
    return Step::Done(base.skip("no HEAD"));
  };
  let status = git.run(&["status", "--porcelain"], None);
  let ahead = git.run(
    &["rev-list", "--count", "HEAD", "--not", "--remotes=origin"],
    None,
  );
  let (Some(status), Some(ahead)) = (status, ahead) else {
    return Step::Done(base.skip("git failed"));
  };
  let dirty = !status.is_empty();
  let Ok(unpushed) = ahead.parse::<u64>() else {
    return Step::Done(
      Snapshot {
        dirty,
        ..base.clone()
      }
      .skip("git failed"),
    );
  };
  if !dirty && unpushed == 0 {
    return Step::Done(base.skip("nothing at risk"));
  }
  let seen = Snapshot {
    dirty,
    unpushed,
    ..base.clone()
  };
  let sha = if dirty {
    match dirty_commit(git, storage, &head, &seen) {
      Ok(sha) => sha,
      Err(step) => return step,
    }
  } else {
    head
  };
  Step::Done(pin(git, seen, sha))
}

/// Point the ref at `sha`, unless it already does.
fn pin(git: &Git<'_>, seen: Snapshot, sha: String) -> Snapshot {
  let current = git.run(&["rev-parse", "--verify", "--quiet", &seen.reference], None);
  if current.as_deref() == Some(sha.as_str()) {
    return Snapshot {
      sha: Some(sha),
      ..seen
    };
  }
  let update = [
    "update-ref",
    "--create-reflog",
    "-m",
    "epic checkpoint",
    &seen.reference,
    &sha,
  ];
  if git.run(&update, None).is_none() {
    return seen.skip("update-ref failed");
  }
  Snapshot {
    sha: Some(sha),
    changed: true,
    ..seen
  }
}

/// The snapshot commit of a dirty tree, or the step that ends the attempt: the
/// unchanged previous snapshot, a failure, or a HEAD that moved.
fn dirty_commit(
  git: &Git<'_>,
  storage: &Storage,
  head: &str,
  seen: &Snapshot,
) -> Result<String, Step> {
  let tree = prepare_tree(git, storage, head).ok_or_else(|| Step::Done(seen.skip("git failed")))?;
  stable_head(git, head, seen)?;
  let current = git.run(&["rev-parse", "--verify", "--quiet", &seen.reference], None);
  if let Some(current) = current
    && previous_matches(git, &current, &tree, head)
  {
    return Err(Step::Done(Snapshot {
      sha: Some(current),
      ..seen.clone()
    }));
  }
  let stamp = toolu_state::time::iso_millis(SystemTime::now());
  let message = format!("epic-wip {} {stamp}", seen.key);
  let commit = git
    .run(&["commit-tree", &tree, "-p", head, "-m", &message], None)
    .ok_or_else(|| Step::Done(seen.skip("commit-tree failed")))?;
  stable_head(git, head, seen)?;
  Ok(commit)
}

fn stable_head(git: &Git<'_>, head: &str, seen: &Snapshot) -> Result<(), Step> {
  match git.run(&["rev-parse", "HEAD"], None) {
    None => Err(Step::Done(seen.skip("no HEAD"))),
    Some(now) if now != head => Err(Step::Retry(seen.skip("HEAD changed during checkpoint"))),
    Some(_) => Ok(()),
  }
}

/// Whether the saved snapshot already holds `tree` on top of `head`.
fn previous_matches(git: &Git<'_>, current: &str, tree: &str, head: &str) -> bool {
  let previous_tree = git.run(&["rev-parse", &format!("{current}^{{tree}}")], None);
  if previous_tree.as_deref() != Some(tree) {
    return false;
  }
  current == head
    || git
      .run(&["rev-parse", &format!("{current}^")], None)
      .as_deref()
      == Some(head)
}

/// Snapshot each running or awaiting-merge issue of `state_dir`, or only
/// `only` (whatever its stage).
pub(crate) fn snapshot_active(
  state_dir: &Path,
  only: Option<&str>,
  trace: Option<&Path>,
) -> Vec<Snapshot> {
  let Ok(entries) = std::fs::read_dir(state_dir.join("issues")) else {
    return Vec::new();
  };
  let mut names: Vec<String> = entries
    .filter_map(Result::ok)
    .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
    .collect();
  names.sort();
  names
    .iter()
    .filter_map(|name| {
      let key = name.strip_suffix(".json")?;
      if only.is_some_and(|only| only != key) {
        return None;
      }
      let record =
        crate::disk::read_value(&state_dir.join("issues").join(name)).unwrap_or_default();
      let stage = record.get("stage").and_then(serde_json::Value::as_str);
      let active = matches!(stage, Some("running" | "awaiting_merge"));
      if only.is_none() && !active {
        return None;
      }
      let worktree = record
        .get("worktree")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
      Some(snapshot(worktree, key, trace))
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/checkpoint_test.rs"]
mod tests;
