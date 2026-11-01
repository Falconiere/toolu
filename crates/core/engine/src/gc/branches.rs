//! Local branches merged into the origin default branch. The engine's
//! unattended sweep deletes only the branches the epic's issue records name;
//! `toolu gc --apply` (`all_branches`) deletes every merged one. A branch
//! checked out in a worktree is kept, in the report as in the apply.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use super::repos::IssueRecord;
use super::worktrees::{default_branch, listed};
use super::{Request, git, git_ok};

/// The merged branches of `root`: `removed` names and `kept` names with reasons.
pub(crate) fn sweep(request: &Request, root: &Path, records: &[IssueRecord]) -> Value {
  let env = &request.env;
  let default = default_branch(env, root);
  let target = format!("refs/remotes/origin/{default}");
  let format = "--format=%(refname:short) %(objectname)";
  let text = match git(env, root, &["for-each-ref", format, "refs/heads/"]) {
    Ok(text) => text,
    Err(err) => return json!({"skipped": err}),
  };
  let checked_out: BTreeSet<String> = listed(env, root)
    .unwrap_or_default()
    .into_iter()
    .filter_map(|tree| tree.branch)
    .collect();
  let recorded: BTreeSet<&str> = records
    .iter()
    .filter_map(|record| record.branch.as_deref())
    .collect();
  let mut removed = Vec::new();
  let mut kept = Vec::new();
  for line in text.lines() {
    let Some((name, sha)) = line.split_once(' ') else {
      continue;
    };
    let in_scope = request.all_branches || recorded.contains(name);
    if name == default
      || !in_scope
      || !git_ok(env, root, &["merge-base", "--is-ancestor", sha, &target])
    {
      continue;
    }
    if checked_out.contains(name) {
      kept.push(json!({"name": name, "reason": "checked out in a worktree"}));
    } else if !request.apply {
      removed.push(name.to_owned());
    } else {
      match git(env, root, &["branch", "--quiet", "-d", name]) {
        Ok(_) => removed.push(name.to_owned()),
        Err(err) => kept.push(json!({"name": name, "reason": err})),
      }
    }
  }
  json!({"removed": removed, "kept": kept})
}

#[cfg(test)]
#[path = "tests/branches_test.rs"]
mod tests;
