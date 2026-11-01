//! Linked worktrees a sweep may remove: toolu-owned, merged into the origin
//! default branch, clean, held by no worker, and not open in herdr. Anything
//! else is kept with its reason, and `git worktree prune` drops the metadata
//! of deleted ones. A squash-merged branch is not an ancestor, so its worktree
//! stays and is listed for the human.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::{Request, capture, git, git_ok};

/// Untracked host session files that do not make a worktree dirty.
const SESSION_FILES: [&str; 4] = [".claude/", ".codex/", ".cursor/", ".toolu/tmp/"];

/// One row of `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tree {
  /// The worktree path.
  pub path: String,
  /// Its `HEAD` commit.
  pub head: String,
  /// Its branch, short; `None` when detached.
  pub branch: Option<String>,
  /// Git lists it but the directory is gone.
  pub prunable: bool,
}

/// What herdr says about open workspaces.
enum Open {
  /// No `herdr` on `PATH`: nothing can be open.
  Absent,
  /// `herdr` is there but did not answer.
  Unreachable(String),
  /// The worktree paths with an open workspace.
  Paths(BTreeSet<String>),
}

/// Parse `git worktree list --porcelain`.
pub(crate) fn parse(text: &str) -> Vec<Tree> {
  let mut trees = Vec::new();
  for block in text.split("\n\n").filter(|block| !block.trim().is_empty()) {
    let mut tree = Tree {
      path: String::new(),
      head: String::new(),
      branch: None,
      prunable: false,
    };
    for line in block.lines() {
      if let Some(path) = line.strip_prefix("worktree ") {
        path.clone_into(&mut tree.path);
      } else if let Some(head) = line.strip_prefix("HEAD ") {
        head.clone_into(&mut tree.head);
      } else if let Some(branch) = line.strip_prefix("branch ") {
        tree.branch = Some(
          branch
            .strip_prefix("refs/heads/")
            .unwrap_or(branch)
            .to_owned(),
        );
      } else if line.starts_with("prunable") {
        tree.prunable = true;
      }
    }
    trees.push(tree);
  }
  trees
}

/// The worktrees of `root` as git lists them, the main one first.
pub(crate) fn listed(env: &Env, root: &Path) -> Result<Vec<Tree>, String> {
  git(env, root, &["worktree", "list", "--porcelain"]).map(|text| parse(&text))
}

/// Whether toolu or one of its workers made the worktree at `path`.
pub(crate) fn owned(path: &str) -> bool {
  let name = Path::new(path)
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_default();
  let hashed = name
    .strip_prefix("toolu-")
    .is_some_and(|rest| rest.len() == 24 && rest.chars().all(|c| c.is_ascii_hexdigit()));
  path.contains("/.herdr/worktrees/") || path.contains("pr-babysit") || hashed
}

/// `origin`'s default branch, short; `main` when `origin/HEAD` is unset.
pub(crate) fn default_branch(env: &Env, root: &Path) -> String {
  git(
    env,
    root,
    &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
  )
  .ok()
  .and_then(|text| text.trim().strip_prefix("origin/").map(str::to_owned))
  .unwrap_or_else(|| "main".to_owned())
}

/// The worktrees of `root`: each linked one's decision, and the pruned count.
/// `held` are worktree paths a lease or a busy issue record still names.
pub(crate) fn sweep(request: &Request, root: &Path, held: &BTreeSet<String>) -> Value {
  let env = &request.env;
  let default = default_branch(env, root);
  let target = format!("refs/remotes/origin/{default}");
  if !git_ok(env, root, &["rev-parse", "--verify", "--quiet", &target]) {
    return json!({"root": root, "skipped": format!("no {target}")});
  }
  if request.apply {
    let _fetched = git(env, root, &["fetch", "--quiet", "origin", &default]);
  }
  let trees = match listed(env, root) {
    Ok(trees) => trees,
    Err(err) => return json!({"root": root, "skipped": err}),
  };
  let open = herdr_open(env, root);
  let rows: Vec<Value> = trees
    .iter()
    .skip(1)
    .map(|tree| judge(request, root, tree, &target, (held, &open)))
    .collect();
  let pruned = trees.iter().filter(|tree| tree.prunable).count();
  if request.apply {
    let _pruned = git(env, root, &["worktree", "prune"]);
  }
  json!({"root": root, "default_branch": default, "worktrees": rows, "pruned": pruned})
}

/// Why `tree` stays, or `None` when it may go.
fn keep_reason(
  env: &Env,
  root: &Path,
  tree: &Tree,
  target: &str,
  (held, open): (&BTreeSet<String>, &Open),
) -> Option<String> {
  if !owned(&tree.path) {
    return Some("not made by toolu".to_owned());
  }
  if held.contains(&tree.path) {
    return Some("a worker holds it".to_owned());
  }
  if !git_ok(
    env,
    root,
    &["merge-base", "--is-ancestor", &tree.head, target],
  ) {
    return Some(format!("not merged into {target}"));
  }
  match dirty(env, Path::new(&tree.path)) {
    Ok(0) => {}
    Ok(count) => return Some(format!("{count} uncommitted changes")),
    Err(err) => return Some(format!("its state is unknown: {err}")),
  }
  match open {
    Open::Unreachable(why) => Some(format!("herdr did not answer: {why}")),
    Open::Paths(paths) if paths.contains(&tree.path) => Some("open in herdr".to_owned()),
    Open::Absent | Open::Paths(_) => None,
  }
}

fn judge(
  request: &Request,
  root: &Path,
  tree: &Tree,
  target: &str,
  fences: (&BTreeSet<String>, &Open),
) -> Value {
  let row = |action: &str, reason: &str| json!({"path": tree.path, "branch": tree.branch, "action": action, "reason": reason});
  if tree.prunable {
    return row("pruned", "the directory is gone");
  }
  if let Some(reason) = keep_reason(&request.env, root, tree, target, fences) {
    return row("kept", &reason);
  }
  if !request.apply {
    return row("would-remove", "merged and clean");
  }
  match git(
    &request.env,
    root,
    &["worktree", "remove", "--force", &tree.path],
  ) {
    Ok(_) => row("removed", "merged and clean"),
    Err(err) => row("kept", &err),
  }
}

/// How many changes `git status` shows, host session files aside.
///
/// # Errors
/// `git status` failed, so the worktree's state is unknown.
fn dirty(env: &Env, path: &Path) -> Result<usize, String> {
  let text = git(
    env,
    path,
    &["status", "--porcelain", "--untracked-files=all"],
  )?;
  Ok(
    text
      .lines()
      .filter(|line| !line.is_empty())
      .filter(|line| {
        !line
          .strip_prefix("?? ")
          .is_some_and(|file| SESSION_FILES.iter().any(|prefix| file.starts_with(prefix)))
      })
      .count(),
  )
}

/// The worktree paths with an open workspace in a `herdr worktree list` reply,
/// whether herdr wrapped it in `result` or not.
pub(crate) fn open_paths(reply: &Value) -> BTreeSet<String> {
  let rows = reply
    .pointer("/result/worktrees")
    .or_else(|| reply.get("worktrees"))
    .and_then(Value::as_array);
  rows
    .into_iter()
    .flatten()
    .filter(|row| {
      row
        .get("open_workspace_id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty())
    })
    .filter_map(|row| row.get("path").and_then(Value::as_str))
    .map(str::to_owned)
    .collect()
}

fn herdr_open(env: &Env, root: &Path) -> Open {
  if !on_path(env, "herdr") {
    return Open::Absent;
  }
  let root = root.to_string_lossy();
  let argv = ["herdr", "worktree", "list", "--cwd", root.as_ref()];
  let text = match capture(env, None, &argv, super::QUERY) {
    Ok(text) => text,
    Err(err) => return Open::Unreachable(err),
  };
  match serde_json::from_str::<Value>(&text) {
    Ok(reply) => Open::Paths(open_paths(&reply)),
    Err(_) => Open::Unreachable("unreadable reply".to_owned()),
  }
}

/// Whether `program` is an executable file on `env`'s `PATH`.
pub(crate) fn on_path(env: &Env, program: &str) -> bool {
  env
    .get("PATH")
    .unwrap_or_default()
    .split(':')
    .filter(|dir| !dir.is_empty())
    .any(|dir| Path::new(dir).join(program).is_file())
}

#[cfg(test)]
#[path = "tests/worktrees_test.rs"]
mod tests;
