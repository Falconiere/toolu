//! The issue's herdr worktree and workspace (`ensureWorktree`): reuse an open
//! one, reopen a listed one, or create it from `origin/<base>`.

use serde_json::Value;

use super::checkout::shell_join;
use super::failure::Failure;
use super::lifecycle::Herdr;
use crate::graph::GraphIssue;
use crate::herdr::Call;

/// The worktree a worker runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Worktree {
  /// The herdr workspace.
  pub workspace_id: String,
  /// The pane the agent starts in.
  pub pane_id: String,
  /// The resolved worktree path.
  pub path: String,
}

/// Where the worktree goes.
pub(crate) struct Place<'a> {
  /// The primary checkout.
  pub checkout: &'a str,
  /// The default branch.
  pub base: &'a str,
  /// List and create nothing; return placeholders.
  pub dry: bool,
}

fn real(path: &str) -> Result<String, Failure> {
  std::fs::canonicalize(path)
    .map(|path| path.display().to_string())
    .map_err(|err| Failure::unknown(format!("{path}: {err}")))
}

fn listed_path(row: &Value, branch: &str) -> Result<String, Failure> {
  row
    .get("path")
    .and_then(Value::as_str)
    .filter(|path| !path.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| Failure::refusal(format!("herdr listed {branch} without a worktree path")))
}

/// Reuse the workspace a listed worktree is already open in.
fn reuse_open(
  herdr: &mut Herdr<'_>,
  row: &Value,
  workspace: &str,
  branch: &str,
  log: &mut Vec<String>,
) -> Result<Worktree, Failure> {
  let panes = herdr.call(&Call::PaneList {
    workspace_id: workspace.to_owned(),
  })?;
  let pane = panes
    .pointer("/panes/0/pane_id")
    .and_then(Value::as_str)
    .ok_or_else(|| Failure::refusal(format!("no panes in workspace {workspace}")))?
    .to_owned();
  log.push(format!("reuse open worktree workspace {workspace}"));
  Ok(Worktree {
    workspace_id: workspace.to_owned(),
    pane_id: pane,
    path: real(&listed_path(row, branch)?)?,
  })
}

/// Ensure the worktree of `issue` at `place`. A dry run lists nothing and
/// returns placeholders.
///
/// # Errors
/// herdr fails, a listed worktree has no path, or an open one has no pane.
pub(crate) fn ensure_worktree(
  herdr: &mut Herdr<'_>,
  place: &Place<'_>,
  issue: &GraphIssue,
  log: &mut Vec<String>,
) -> Result<Worktree, Failure> {
  let listed = if place.dry {
    Vec::new()
  } else {
    let reply = herdr.call(&Call::WorktreeList {
      cwd: place.checkout.to_owned(),
    })?;
    reply
      .get("worktrees")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default()
  };
  let existing = listed
    .iter()
    .find(|row| row.get("branch").and_then(Value::as_str) == Some(issue.branch.as_str()));
  let open = existing
    .and_then(|row| row.get("open_workspace_id"))
    .and_then(Value::as_str)
    .filter(|id| !id.is_empty());
  if let (Some(row), Some(workspace)) = (existing, open) {
    return reuse_open(herdr, row, workspace, &issue.branch, log);
  }
  let call = open_or_create(place, issue, existing)?;
  log.push(shell_join(&call.argv(herdr.env)));
  if place.dry {
    return Ok(Worktree {
      workspace_id: "<new>".to_owned(),
      pane_id: "<root-pane>".to_owned(),
      path: "<herdr worktree path>".to_owned(),
    });
  }
  let reply = herdr.call(&call)?;
  opened(&reply, &call)
}

/// Reopen a listed worktree, or create one from `origin/<base>`.
fn open_or_create(
  place: &Place<'_>,
  issue: &GraphIssue,
  existing: Option<&Value>,
) -> Result<Call, Failure> {
  Ok(match existing {
    Some(row) => Call::WorktreeOpen {
      cwd: place.checkout.to_owned(),
      path: listed_path(row, &issue.branch)?,
      label: issue.key.clone(),
    },
    None => Call::Create {
      cwd: Some(place.checkout.to_owned()),
      branch: Some(issue.branch.clone()),
      base: Some(format!("origin/{}", place.base)),
      path: None,
      label: Some(issue.key.clone()),
      workspace: None,
    },
  })
}

/// The workspace, root pane and resolved path of an opened worktree.
fn opened(reply: &Value, call: &Call) -> Result<Worktree, Failure> {
  let field = |pointer: &str| {
    reply
      .pointer(pointer)
      .and_then(Value::as_str)
      .map(str::to_owned)
  };
  let (Some(workspace_id), Some(pane_id), Some(path)) = (
    field("/workspace/workspace_id"),
    field("/root_pane/pane_id"),
    field("/worktree/path"),
  ) else {
    return Err(Failure::refusal(format!(
      "herdr {} omitted the workspace, pane or path",
      call.method()
    )));
  };
  Ok(Worktree {
    workspace_id,
    pane_id,
    path: real(&path)?,
  })
}

#[cfg(test)]
#[path = "tests/worktree_test.rs"]
mod tests;
