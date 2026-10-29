//! Safe teardown of a remote hc worktree.

use super::hc::stop;
use super::ownership::{fence_cleanup_ownership, verify_no_jobs};
use super::remove::Teardown;
use crate::launch::Ctx;
use crate::launch::record::{Record, text, write_record};
use serde_json::{Value, json};
use toolu_mux::hc::remote::{Target, Transfer};

/// Match an exact Git worktree path and branch from porcelain -z output.
pub(super) fn registered(listing: &str, path: &str, branch: &str) -> Result<bool, String> {
  let mut found = false;
  let mut selected = false;
  let mut matches_branch = false;
  for field in listing.split('\0') {
    if let Some(tree) = field.strip_prefix("worktree ") {
      if selected && !matches_branch {
        return Err("remote worktree branch differs from the recorded branch".to_owned());
      }
      selected = tree == path;
      matches_branch = false;
      found |= selected;
    } else if selected && field == format!("branch refs/heads/{branch}") {
      matches_branch = true;
    }
  }
  if selected && !matches_branch {
    return Err("remote worktree branch differs from the recorded branch".to_owned());
  }
  Ok(found)
}

/// Stop the owned remote worker and reconcile its worktree removal.
pub(super) fn remote_teardown(
  ctx: &Ctx<'_>,
  teardown: &Teardown<'_>,
  state: &str,
  key: &str,
  record: &mut Record,
) -> Result<(), String> {
  if !teardown.abandon && record.get("remote_result_imported") != Some(&Value::Bool(true)) {
    return Err("remote result has not been verified and imported".to_owned());
  }
  let machine = text(record, "machine")
    .ok_or("missing remote machine")?
    .to_owned();
  let repo = text(record, "remote_repo")
    .ok_or("missing remote repository")?
    .to_owned();
  let tree = text(record, "remote_worktree")
    .ok_or("missing remote worktree")?
    .to_owned();
  let run = text(record, "hc_run")
    .ok_or("missing hc run identity")?
    .to_owned();
  let branch = text(record, "branch").ok_or("missing branch")?.to_owned();
  let target = Target::new(Some(&machine), Some(&repo))
    .map_err(|err| err.message)?
    .ok_or("missing remote target")?;
  Target::new(Some(&machine), Some(&tree)).map_err(|err| err.message)?;
  if tree == repo {
    return Err("remote worktree is the primary checkout".to_owned());
  }
  let transfer = Transfer {
    target,
    env: ctx.env,
    run: &run,
  };
  let ownership = fence_cleanup_ownership(teardown.path, state, key, record)?;
  let local_tree = text(record, "worktree").unwrap_or("").to_owned();
  verify_no_jobs(ownership.as_ref(), &local_tree)?;
  let address = stop_remote(ctx, record)?;
  let intent = json!({"address": address, "machine": machine, "repo": repo, "worktree": tree, "branch": branch});
  remove(&transfer, teardown, record, &intent)
}

/// A local delivery worker has a separate address from its remote implementer.
fn stop_remote(ctx: &Ctx<'_>, record: &Record) -> Result<String, String> {
  let mut remote = record.clone();
  if let Some(address) = text(record, "remote_terminal_address") {
    remote.insert("terminal_address".into(), json!(address));
  }
  stop(ctx, &remote)
}

fn check_worktree(
  transfer: &Transfer<'_>,
  tree: &str,
  record: &Record,
  abandon: bool,
) -> Result<(), String> {
  let git = |args: &[&str]| transfer.git(args).map_err(|err| err.message);
  if !git(&["-C", tree, "status", "--porcelain", "--untracked-files=all"])?.is_empty() {
    return Err("remote worktree has uncommitted work; preserve it for inspection".to_owned());
  }
  if !abandon
    && git(&["-C", tree, "rev-parse", "HEAD"])?
      != text(record, "remote_result_commit").ok_or("missing imported remote commit")?
  {
    return Err("remote worktree has untransferred commits; preserve it for inspection".to_owned());
  }
  Ok(())
}

fn remove(
  transfer: &Transfer<'_>,
  teardown: &Teardown<'_>,
  record: &mut Record,
  intent: &Value,
) -> Result<(), String> {
  let tree = intent
    .get("worktree")
    .and_then(Value::as_str)
    .ok_or("missing remote worktree")?;
  let branch = intent
    .get("branch")
    .and_then(Value::as_str)
    .ok_or("missing branch")?;
  let git = |args: &[&str]| {
    transfer
      .git(args)
      .map_err(|err| format!("remote git {}: {}", err.code, err.message))
  };
  if let Some(saved) = record.get("remote_removal_intent")
    && saved != intent
  {
    return Err("remote removal intent does not match recorded ownership".to_owned());
  }
  let listing = git(&["worktree", "list", "--porcelain", "-z"])?;
  if registered(&listing, tree, branch)? {
    check_worktree(transfer, tree, record, teardown.abandon)?;
    record.insert("stage".to_owned(), json!("cleanup-incomplete"));
    record.insert("remote_removal_intent".to_owned(), intent.clone());
    write_record(teardown.path, record)?;
    git(&["worktree", "remove", "--", tree])?;
  } else {
    if record.get("remote_removal_intent") != Some(intent) {
      return Err("remote worktree removal is uncertain; inspect it before cleanup".to_owned());
    }
    for flag in ["-e", "-L"] {
      transfer
        .exec(&["test", "!", flag, tree], &[])
        .map_err(|error| {
          format!(
            "remote worktree path still exists or cannot be verified: {}",
            error.message
          )
        })?;
    }
  }
  finalize(transfer, teardown, record, tree, branch)
}

fn finalize(
  transfer: &Transfer<'_>,
  teardown: &Teardown<'_>,
  record: &mut Record,
  tree: &str,
  branch: &str,
) -> Result<(), String> {
  let git = |args: &[&str]| {
    transfer
      .git(args)
      .map_err(|err| format!("remote git {}: {}", err.code, err.message))
  };
  let listing = git(&["worktree", "list", "--porcelain", "-z"])?;
  if registered(&listing, tree, branch)? {
    return Err("remote worktree remains registered".to_owned());
  }
  record.insert("remote_worktree_removed".to_owned(), json!(true));
  write_record(teardown.path, record)?;
  if !teardown.abandon {
    let reference = format!("refs/heads/{branch}");
    if listing
      .split('\0')
      .any(|field| field == format!("branch {reference}"))
    {
      return Err("remote branch is checked out in another worktree".to_owned());
    }
    let refs = git(&["for-each-ref", "--format=%(refname)", &reference])?;
    if refs.lines().any(|line| line == reference) {
      let commit = text(record, "remote_result_commit").ok_or("missing imported remote commit")?;
      git(&["update-ref", "-d", &reference, commit])?;
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/hc_remote_test.rs"]
mod tests;
