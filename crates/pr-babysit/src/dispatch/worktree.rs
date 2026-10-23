//! The slot's fixer worktree on `pr-babysit/<slot>`: reused and fast-forwarded,
//! created by herdr with a pane.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use super::dispatcher::Dispatcher;
use super::herdr::{argv, command, herdr, try_herdr};
use crate::error::{Failure, Result};
use crate::pattern::{is, pattern};

// Muse writes nothing into the worktree; toolu's hooks under Muse keep their
// state in `.toolu/tmp/`.
pattern!(SESSION_FILES, r"^\?\? \.(claude|codex|cursor|toolu/tmp)/");

/// The worktree's uncommitted changes, host session files left out.
pub(super) fn dirt(dispatcher: &Dispatcher<'_>, path: &str) -> Vec<String> {
  let status = argv(&[
    "git",
    "-C",
    path,
    "status",
    "--porcelain",
    "--untracked-files=all",
  ]);
  let ran = command(dispatcher.env, &status, Duration::from_secs(120));
  let lines = ran
    .output
    .lines()
    .filter(|line| !line.is_empty() && !is(&SESSION_FILES, line));
  lines.map(str::to_owned).collect()
}

/// # Errors
/// `worktree_dirty` naming the changes.
pub(super) fn clean_or_fail(dispatcher: &Dispatcher<'_>, path: &str) -> Result<()> {
  let changes = dirt(dispatcher, path);
  if changes.is_empty() {
    return Ok(());
  }
  Err(
    Failure::new(
      "worktree_dirty",
      format!("fixer worktree has uncommitted changes: {path}"),
    )
    .with("path", path)
    .with("changes", changes),
  )
}

struct Pane {
  workspace: Value,
  pane: Value,
}

fn id(value: &Value, pointer: &str) -> Value {
  value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

impl Dispatcher<'_> {
  fn drop_branch(&mut self, root: &str, branch: &str, pr: &str) -> Result<()> {
    if self.dry
      || !self.probe(&argv(&[
        "git",
        "-C",
        root,
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("refs/heads/{branch}"),
      ]))
    {
      return Ok(());
    }
    let ancestor = argv(&[
      "git",
      "-C",
      root,
      "merge-base",
      "--is-ancestor",
      &format!("refs/heads/{branch}"),
      &format!("refs/remotes/origin/{pr}"),
    ]);
    if !self.probe(&ancestor) {
      let message = format!(
        "local {branch} holds commits origin/{pr} does not; inspect it before babysit reuses the name"
      );
      return Err(Failure::new("stale_branch", message).with("branch", branch));
    }
    let message = format!("git could not delete {branch} (is it checked out in another worktree?)");
    self.git(
      &argv(&["git", "-C", root, "branch", "--quiet", "-D", branch]),
      "stale_branch",
      message,
    )?;
    Ok(())
  }

  fn open_pane(&mut self, root: &str, path: &str, label: &str) -> Result<Pane> {
    let open = argv(&[
      "herdr",
      "worktree",
      "open",
      "--cwd",
      root,
      "--path",
      path,
      "--label",
      label,
      "--no-focus",
    ]);
    self.record(&open);
    if self.dry {
      return Ok(Pane {
        workspace: json!("<workspace>"),
        pane: json!("<root pane>"),
      });
    }
    let opened = herdr(self.env, open.get(1..).unwrap_or_default())?;
    Ok(Pane {
      workspace: id(&opened, "/workspace/workspace_id"),
      pane: id(&opened, "/root_pane/pane_id"),
    })
  }

  fn pane_listed(&self, workspace: &Value, pane: &Value) -> bool {
    let Some(workspace) = workspace.as_str() else {
      return false;
    };
    let listed = try_herdr(
      self.env,
      &argv(&["pane", "list", "--workspace", workspace]),
      Duration::from_secs(120),
    )
    .ok();
    let panes = listed
      .as_ref()
      .and_then(|value| value.get("panes")?.as_array());
    panes.is_some_and(|panes| panes.iter().any(|entry| entry.get("pane_id") == Some(pane)))
  }

  fn reuse(&mut self, existing: &str, pr: &str, branch: &str) -> Result<()> {
    if !self.dry {
      clean_or_fail(self, existing)?;
    }
    let fetch = argv(&["git", "-C", existing, "fetch", "--quiet", "origin", pr]);
    self.git(
      &fetch,
      "git_error",
      format!("git fetch origin {pr} failed in {existing}"),
    )?;
    let merge = argv(&[
      "git",
      "-C",
      existing,
      "merge",
      "--quiet",
      "--ff-only",
      &format!("refs/remotes/origin/{pr}"),
    ]);
    let message = format!(
      "{branch} cannot fast-forward to origin/{pr} (the PR branch was rewritten); run dispatch-fix cleanup, then start again"
    );
    self.git(&merge, "stale_branch", message)?;
    Ok(())
  }
}

impl Dispatcher<'_> {
  fn label(&self) -> String {
    format!(
      "pb-{}",
      self
        .state
        .get("number")
        .and_then(Value::as_u64)
        .unwrap_or_default()
    )
  }

  fn create_herdr(&mut self, root: &str, branch: &str, base: &str) -> Result<(String, Pane)> {
    let label = self.label();
    let create = argv(&[
      "herdr",
      "worktree",
      "create",
      "--cwd",
      root,
      "--branch",
      branch,
      "--base",
      base,
      "--label",
      &label,
      "--no-focus",
    ]);
    self.record(&create);
    if self.dry {
      let pane = Pane {
        workspace: json!("<workspace>"),
        pane: json!("<root pane>"),
      };
      return Ok(("<herdr worktree path>".into(), pane));
    }
    let created = herdr(self.env, create.get(1..).unwrap_or_default())?;
    let path = created
      .pointer("/worktree/path")
      .and_then(Value::as_str)
      .filter(|path| !path.is_empty())
      .ok_or_else(|| {
        Failure::new(
          "herdr_error",
          "herdr worktree create returned no worktree.path",
        )
      })?;
    let pane = Pane {
      workspace: id(&created, "/workspace/workspace_id"),
      pane: id(&created, "/root_pane/pane_id"),
    };
    Ok((path.to_owned(), pane))
  }

  fn create(&mut self, root: &str, pr: &str, branch: &str) -> Result<(String, Pane)> {
    let fetch = argv(&["git", "-C", root, "fetch", "--quiet", "origin", pr]);
    self.git(
      &fetch,
      "git_error",
      format!("git fetch origin {pr} failed in {root}"),
    )?;
    let prune = argv(&["git", "-C", root, "worktree", "prune"]);
    self.git(
      &prune,
      "git_error",
      format!("git -C {root} worktree prune failed"),
    )?;
    self.drop_branch(root, branch, pr)?;
    self.create_herdr(root, branch, &format!("origin/{pr}"))
  }

  /// The slot worktree for PR branch `pr` in `root`, recorded in the state.
  pub(super) fn worktree(&mut self, root: &str, pr: &str) -> Result<Value> {
    let branch = format!(
      "pr-babysit/{}",
      self
        .state
        .get("slot")
        .and_then(Value::as_str)
        .unwrap_or_default()
    );
    let existing = self.worktree_field("path").map(str::to_owned);
    let reusable = existing.filter(|path| self.dry || Path::new(path).exists());
    let (path, pane) = match reusable {
      Some(path) => {
        self.reuse(&path, pr, &branch)?;
        let recorded = self
          .state
          .get("herdrWorktree")
          .cloned()
          .unwrap_or(Value::Null);
        let mut pane = Pane {
          workspace: id(&recorded, "/workspaceId"),
          pane: id(&recorded, "/paneId"),
        };
        if !self.dry && !self.pane_listed(&pane.workspace, &pane.pane) {
          let label = self.label();
          pane = self.open_pane(root, &path, &label)?;
        }
        (path, pane)
      }
      None => self.create(root, pr, &branch)?,
    };
    let worktree = json!({
      "path": path, "workspaceId": pane.workspace, "paneId": pane.pane, "branch": branch,
      "prBranch": pr, "repoRoot": root, "base": format!("origin/{pr}"),
    });
    let recorded = worktree.clone();
    self.save(|state| {
      if let Value::Object(root) = state {
        root.insert("herdrWorktree".into(), recorded);
      }
    })?;
    Ok(worktree)
  }
}

#[cfg(test)]
#[path = "tests/worktree_test.rs"]
mod tests;
