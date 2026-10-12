//! The engine's mechanical rebase of a merge-queue head (#448): in the
//! parked worker's worktree, under the resource-lease fence, after a
//! snapshot to `refs/epic-wip/<key>`, a plain `git rebase origin/<base>`
//! pushed with `--force-with-lease`. Anything unexpected (a dirty tracked
//! file, another branch, local commits the PR lacks, a conflict, a refused
//! push) hands the rebase to the worker; a conflict is aborted first, so the
//! worktree is exactly as before.

use std::path::{Path, PathBuf};

mod git;

use git::Git;
use toolu_runtime::env::Env;

/// What a rebase works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
  /// The worker's worktree.
  pub worktree: PathBuf,
  /// The issue key, for the snapshot ref.
  pub key: String,
  /// The PR's head branch.
  pub branch: String,
  /// The PR's base branch.
  pub base: String,
  /// The PR head SHA the gate verified.
  pub head: String,
}

/// The agent lease that fences the worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lease {
  /// The resource root.
  pub root: PathBuf,
  /// The agent lease token.
  pub token: String,
}

/// How a rebase ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rebased {
  /// Rebased and pushed; the new head.
  Pushed(String),
  /// Not safe for the engine; the worker rebases. The reason.
  Handoff(String),
  /// Not now (a job runs there, the snapshot failed); retried at the next check.
  Busy(String),
}

/// Rebase `target` onto `origin/<base>` and push it, fenced by `lease`.
pub(crate) fn rebase(
  target: &Target,
  lease: Option<&Lease>,
  env: &Env,
  trace: Option<&Path>,
) -> Rebased {
  let git = Git::new(&target.worktree, env);
  if let Err(reason) = git::preconditions(&git, target) {
    return Rebased::Handoff(reason);
  }
  if let Some(lease) = lease
    && let Err(reason) = fence(lease, &target.worktree)
  {
    return Rebased::Busy(reason);
  }
  let outcome = snapshot(&git, target, trace).map_or_else(Rebased::Busy, |()| replay(&git, target));
  if let Some(lease) = lease
    && let Err(reason) = set_stage(lease, "running")
  {
    return match outcome {
      Rebased::Pushed(head) => Rebased::Busy(format!("pushed {head}; unfence failed: {reason}")),
      Rebased::Handoff(note) => Rebased::Handoff(format!("{note}; unfence failed: {reason}")),
      Rebased::Busy(note) => Rebased::Busy(format!("{note}; unfence failed: {reason}")),
    };
  }
  outcome
}

/// Fetch, rebase, abort on failure, push on success.
fn replay(git: &Git<'_>, target: &Target) -> Rebased {
  if let Err(reason) = git.ok(&["fetch", "--quiet", "origin", &target.base]) {
    return Rebased::Busy(format!("fetch failed: {reason}"));
  }
  let upstream = format!("origin/{}", target.base);
  if let Err(reason) = git.ok(&["rebase", "--quiet", &upstream]) {
    return Rebased::Handoff(abort(git, target, &reason));
  }
  let new_head = match git.out(&["rev-parse", "HEAD"]) {
    Ok(head) => head,
    Err(reason) => return Rebased::Handoff(format!("rebased, but HEAD unreadable: {reason}")),
  };
  if new_head == target.head {
    return Rebased::Busy("already up to date locally; waiting for GitHub".to_owned());
  }
  let lease = format!(
    "--force-with-lease=refs/heads/{}:{}",
    target.branch, target.head
  );
  let refspec = format!("HEAD:refs/heads/{}", target.branch);
  match git.ok(&["push", "--quiet", &lease, "origin", &refspec]) {
    Ok(()) => Rebased::Pushed(new_head),
    Err(reason) => Rebased::Handoff(format!("push refused: {reason}")),
  }
}

/// `git rebase --abort`, then prove the worktree is back where it was.
fn abort(git: &Git<'_>, target: &Target, reason: &str) -> String {
  let aborted = git.ok(&["rebase", "--abort"]);
  let head = git.out(&["rev-parse", "HEAD"]);
  let clean = git.out(&["status", "--porcelain", "--untracked-files=no"]);
  let first = reason.lines().next().unwrap_or("rebase failed");
  match (aborted, head, clean) {
    (Ok(()), Ok(head), Ok(clean)) if head == target.head && clean.is_empty() => {
      format!("conflict: {first}")
    }
    _ => format!(
      "conflict: {first}; abort unverified, snapshot at refs/epic-wip/{}",
      target.key
    ),
  }
}

/// Snapshot the worktree; a clean, pushed one pins `refs/epic-wip/<key>` to HEAD.
fn snapshot(git: &Git<'_>, target: &Target, trace: Option<&Path>) -> Result<(), String> {
  let tree = target.worktree.display().to_string();
  let shot = crate::checkpoint::snapshot(&tree, &target.key, trace);
  match shot.skipped.as_deref() {
    None => Ok(()),
    Some("nothing at risk") => git::pin_head(git, &target.key, &target.head),
    Some(reason) => Err(format!("snapshot failed: {reason}")),
  }
}

/// After an engine restart: a lease the engine left fenced (`rebasing`) means
/// a rebase was cut short. Abort a rebase still in progress in the worktree,
/// then lift the fence.
///
/// # Errors
/// The lease cannot be read or restored, or the abort fails.
pub(crate) fn recover(
  worktree: Option<&Path>,
  lease: Option<&Lease>,
  env: &Env,
) -> Result<(), String> {
  let Some(lease) = lease else {
    return Ok(());
  };
  if lease_stage(lease)?.as_deref() != Some("rebasing") {
    return Ok(());
  }
  if let Some(tree) = worktree.filter(|tree| tree.is_dir()) {
    let git = Git::new(tree, env);
    if git::rebasing(&git)? {
      git.ok(&["rebase", "--abort"])?;
    }
  }
  set_stage(lease, "running")
}

fn lease_stage(lease: &Lease) -> Result<Option<String>, String> {
  use toolu_runtime::json::ordered::Ordered;
  let state = toolu_engine::resources::store::read_resource_state(&lease.root)?;
  let found = toolu_engine::resources::store::leases(&state)
    .iter()
    .find(|item| matches!(item.get("token"), Some(Ordered::String(token)) if *token == lease.token))
    .and_then(|item| match item.get("stage") {
      Some(Ordered::String(stage)) => Some(stage.clone()),
      _ => None,
    });
  Ok(found)
}

/// Stop new managed jobs in `worktree`, then refuse while one still runs.
fn fence(lease: &Lease, worktree: &Path) -> Result<(), String> {
  set_stage(lease, "rebasing")?;
  let tree = worktree.display().to_string();
  let active = toolu_engine::resources::leases::reconcile_resource_jobs(&lease.root)
    .and_then(|_| toolu_engine::resources::transitions::active_jobs(&lease.root, &tree));
  match active {
    Ok(false) => Ok(()),
    Ok(true) => {
      set_stage(lease, "running")?;
      Err("a managed job runs in the worktree".to_owned())
    }
    Err(reason) => {
      set_stage(lease, "running")?;
      Err(reason)
    }
  }
}

fn set_stage(lease: &Lease, stage: &str) -> Result<(), String> {
  let patch = toolu_engine::resources::leases::LeasePatch {
    stage: Some(stage.to_owned()),
    ..Default::default()
  };
  toolu_engine::resources::leases::patch_lease(&lease.root, &lease.token, &patch)
}

#[cfg(test)]
#[path = "tests/rebase_test.rs"]
mod tests;
