//! `dispatch-fix start`: check the plan, prepare the slot worktree, record the
//! round, and launch its first group.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::dispatcher::{Dispatcher, now};
use super::herdr::{argv, reachable};
use super::launch::Launch;
use super::plan::{read_object, validate};
use crate::error::{Failure, Result};
use crate::state::sibling;

/// `start`'s inputs.
pub(crate) struct Start<'a> {
  /// The route plan.
  pub(crate) plan: &'a Path,
  /// The items the plan routes.
  pub(crate) items: &'a Path,
  /// The repository the PR branch lives in.
  pub(crate) repo_root: &'a str,
  /// The PR branch.
  pub(crate) branch: &'a str,
  /// The PR's base branch.
  pub(crate) base: &'a str,
}

fn text<'v>(value: &'v Value, key: &str) -> Option<&'v str> {
  value.get(key).and_then(Value::as_str)
}

impl Dispatcher<'_> {
  fn refuse_active(&self) -> Result<()> {
    let status = self
      .fixer_field("status")
      .and_then(Value::as_str)
      .unwrap_or_default();
    if !matches!(status, "running" | "blocked") {
      return Ok(());
    }
    let groups = self.fixer_field("groups").and_then(Value::as_array);
    let active = groups.and_then(|groups| {
      groups.iter().find(|group| {
        matches!(
          text(group, "status"),
          Some("running" | "launching" | "blocked")
        )
      })
    });
    Err(
      Failure::new(
        "fixer_running",
        "a fixer is already active for this slot; run dispatch-fix wait",
      )
      .with(
        "group",
        self.fixer_field("current").cloned().unwrap_or(Value::Null),
      )
      .with(
        "agent",
        active
          .and_then(|group| group.get("agent"))
          .cloned()
          .unwrap_or(Value::Null),
      ),
    )
  }

  fn checked(&self, start: &Start<'_>) -> Result<(Value, Value)> {
    let plan_message = format!("plan is missing or not JSON: {}", start.plan.display());
    let plan = read_object(start.plan, "plan_invalid", plan_message)?;
    let items_message = format!(
      "items file is missing or not JSON: {}",
      start.items.display()
    );
    let items = read_object(start.items, "plan_invalid", items_message)?;
    validate(&plan, &items, &self.state)?;
    self.refuse_active()?;
    Ok((plan, items))
  }

  fn context(&self, start: &Start<'_>, worktree: &Value, round: u64, groups: usize) -> Value {
    let repo = text(&self.state, "repo").unwrap_or_default();
    let number = self.state.get("number").cloned().unwrap_or(Value::Null);
    let field = |key: &str| worktree.get(key).cloned().unwrap_or(Value::Null);
    json!({
      "pr": format!("{repo}#{number}"), "round": round, "groups": groups,
      "worktree": field("path"), "slotBranch": field("branch"),
      "branch": start.branch, "base": start.base,
    })
  }

  fn record_round(&mut self, plan: &Value, context: &Value, copy: &Path) -> Result<()> {
    let groups = plan
      .get("groups")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    let ids: Vec<Value> = groups
      .iter()
      .flat_map(|group| {
        group
          .get("items")
          .and_then(Value::as_array)
          .cloned()
          .unwrap_or_default()
      })
      .collect();
    let recorded: Vec<Value> = groups
      .iter()
      .map(|group| {
        let field = |key: &str| group.get(key).cloned().unwrap_or(Value::Null);
        json!({"seq": field("seq"), "tier": field("tier"), "host": field("host"),
        "model": field("model"), "effort": field("effort"), "items": field("items"),
        "status": "pending", "reason": null})
      })
      .collect();
    let fixer = json!({
      "round": context.get("round"), "status": "running", "reason": null, "startedAt": now(),
      "current": 1, "unattended": plan.get("unattended") != Some(&Value::Bool(false)),
      "context": context, "itemsFile": copy.display().to_string(), "items": ids,
      "groups": recorded,
    });
    self.save(|state| {
      if let Value::Object(root) = state {
        root.insert("fixer".into(), fixer);
      }
    })
  }

  fn copy_items(&self, start: &Start<'_>) -> Result<PathBuf> {
    let copy = sibling(self.file, ".fixer-items.json");
    if self.dry {
      return Ok(start.items.to_path_buf());
    }
    std::fs::copy(start.items, &copy).map_err(|error| {
      Failure::new(
        "io_error",
        format!("cannot copy {}: {error}", start.items.display()),
      )
    })?;
    Ok(copy)
  }

  /// Starts the round `start` describes.
  ///
  /// # Errors
  /// Plan, config, herdr and git failures, each before the step it guards.
  pub(crate) fn start(&mut self, start: &Start<'_>) -> Result<Value> {
    if [start.base, start.branch]
      .iter()
      .any(|name| name.starts_with('-'))
    {
      return Err(Failure::usage("--base and --branch must not start with -"));
    }
    let (plan, items) = self.checked(start)?;
    let groups = plan
      .get("groups")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default();
    if !self.dry && !reachable(self.env) {
      let message = "herdr is not reachable (not installed, or its server is not running); \
                     run this round inline";
      return Err(Failure::new("herdr_unavailable", message));
    }
    let (root, base) = (start.repo_root, start.base);
    let fetch = argv(&["git", "-C", root, "fetch", "--quiet", "origin", base]);
    self.git(
      &fetch,
      "git_error",
      format!("git fetch origin {base} failed in {root}"),
    )?;
    let worktree = self.worktree(root, start.branch)?;
    let round = crate::json::round(&items).unwrap_or(1);
    let context = self.context(start, &worktree, round, groups.len());
    let items_file = self.copy_items(start)?;
    self.record_round(&plan, &context, &sibling(self.file, ".fixer-items.json"))?;
    let launch = Launch {
      seq: 1,
      items_file: &items_file,
      context: &context,
      pane: text(&worktree, "paneId").unwrap_or_default(),
      unattended: plan.get("unattended") != Some(&Value::Bool(false)),
    };
    let first = if self.dry { groups.first() } else { None };
    let brief = self.launch(&launch, first)?;
    if self.dry {
      return Ok(json!({"dryRun": true, "commands": self.commands, "brief": brief}));
    }
    Ok(self.status())
  }
}

#[cfg(test)]
#[path = "tests/start_test.rs"]
mod tests;
