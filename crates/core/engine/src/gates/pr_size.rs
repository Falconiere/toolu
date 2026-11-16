//! Keep the branch a `git push` sends within the reviewable size budget.

use std::path::Path;

use serde_json::Value;
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::config::read::section;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::git::base_branch;

use super::decided;
use super::push_target::{PushTarget, active_push, git_at, ref_exists};
use crate::verdict::glob::matches_any;

/// The pr-size built-in.
pub(crate) struct PrSize;
/// Its singleton in the pre-tool table.
pub(crate) static PR_SIZE: PrSize = PrSize;

/// `gates.prSize.maxLines` when unset.
const DEFAULT_MAX_LINES: u64 = 300;
/// `gates.prSize.maxFiles` when unset.
const DEFAULT_MAX_FILES: u64 = 100;

/// Paths whose lines are not code lines: generated files, tests and fixtures,
/// and prose. A `gates.prSize.exclude` array replaces the list.
const NOT_CODE: &[&str] = &[
  "*.lock",
  "*-lock.json",
  "*-lock.yaml",
  "go.sum",
  "*/go.sum",
  "*.snap",
  "*.min.js",
  "*.min.css",
  "tests/*",
  "*/tests/*",
  "test/*",
  "*/test/*",
  "*/__tests__/*",
  "*_test.*",
  "*.test.*",
  "*.spec.*",
  "test_*.py",
  "*/test_*.py",
  "fixtures/*",
  "*/fixtures/*",
  "*/testdata/*",
  "*.md",
  "*.mdx",
  "*.rst",
];

/// A branch's code lines and changed files against their budgets.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Size {
  /// Added lines outside the excluded paths; deletions are free.
  pub(crate) added: u64,
  /// The line budget.
  pub(crate) max: u64,
  /// Every changed file, excluded or not.
  pub(crate) files: u64,
  /// The file budget.
  pub(crate) max_files: u64,
  /// The ref the branch was measured from.
  pub(crate) base: String,
}

impl Size {
  /// Whether the branch is over either budget.
  pub(crate) fn over(&self) -> bool {
    self.added > self.max || self.files > self.max_files
  }

  /// `212 of 300 code lines, 14 of 100 files against origin/main`.
  pub(crate) fn summary(&self) -> String {
    format!(
      "{} of {} code lines, {} of {} files against {}",
      self.added, self.max, self.files, self.max_files, self.base
    )
  }
}

fn setting<'a>(config: &'a LoadedConfig, key: &str) -> Option<&'a Value> {
  section(config, "gates")?.get("prSize")?.get(key)
}

fn limit(config: &LoadedConfig, key: &str, fallback: u64) -> u64 {
  let configured = setting(config, key).and_then(Value::as_u64);
  configured.filter(|max| *max > 0).unwrap_or(fallback)
}

/// The code lines and changed files of `git diff --numstat` output. A binary
/// file (`-`) or an excluded path is a changed file with no code lines.
fn counted(numstat: &str, config: &LoadedConfig) -> (u64, u64) {
  let configured = setting(config, "exclude").and_then(Value::as_array);
  let excludes: Vec<String> = match configured {
    Some(globs) => globs
      .iter()
      .filter_map(Value::as_str)
      .map(str::to_owned)
      .collect(),
    None => NOT_CODE.iter().map(|glob| (*glob).to_owned()).collect(),
  };
  let mut total = (0, 0);
  for line in numstat.lines() {
    let mut fields = line.splitn(3, '\t');
    let (Some(added), Some(path)) = (fields.next(), fields.nth(1)) else {
      continue;
    };
    total.1 += 1;
    if !matches_any(path, &excludes) {
      total.0 += added.parse::<u64>().unwrap_or(0);
    }
  }
  total
}

/// The ref a branch is measured from: a non-empty `PUSH_REVIEW_BASE`, else the
/// base's remote-tracking ref when present, else the local base. A local base
/// shared by linked worktrees lags its remote, and would count merged work.
fn size_base(root: &Path, env: &Env, base: &str) -> String {
  if let Some(pinned) = env.get("PUSH_REVIEW_BASE").filter(|base| !base.is_empty()) {
    return pinned.to_owned();
  }
  let remote = format!("origin/{base}");
  if ref_exists(root, &remote, env) {
    remote
  } else {
    base.to_owned()
  }
}

/// The size of the checked-out branch, or none when Git cannot diff it.
pub(crate) fn branch_size(root: &Path, env: &Env, config: &LoadedConfig) -> Option<Size> {
  let base = size_base(root, env, &base_branch(env, Some(root), root));
  let range = format!("{base}...HEAD");
  let numstat = git_at(
    root,
    &["-c", "core.quotepath=false", "diff", "--numstat", &range],
    env,
  )?;
  let (added, files) = counted(&numstat, config);
  Some(Size {
    added,
    max: limit(config, "maxLines", DEFAULT_MAX_LINES),
    files,
    max_files: limit(config, "maxFiles", DEFAULT_MAX_FILES),
    base,
  })
}

fn message(size: &Size, mode: GateMode) -> String {
  let consequence = match mode {
    GateMode::Block => {
      "Push denied until the branch fits; raising the budget is the user's call, not the agent's."
    }
    GateMode::Ask => "Approve to push anyway, or split first.",
    GateMode::Advise | GateMode::Off => "Advisory only — this does not block the push.",
  };
  format!(
    "pr-size: this branch is over its review budget: {} (`gates.prSize.maxLines` and `maxFiles`; tests, fixtures, docs and generated files add no code lines, see `gates.prSize.exclude`). Stop adding to it and split at a coherent boundary: keep the first slice that works alone with its tests, and move the rest to a follow-up branch or issue. {consequence}",
    size.summary()
  )
}

fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Result<Decision, String> {
  let Some((PushTarget { root, branch }, config, mode)) =
    active_push(event, ctx, "prSize", warnings)
  else {
    return Ok(Decision::Allow);
  };
  if branch == base_branch(ctx.env, Some(&root), &root) {
    return Ok(Decision::Allow);
  }
  match branch_size(&root, ctx.env, &config).filter(Size::over) {
    Some(size) => decided(mode, message(&size, mode)),
    None => Ok(Decision::Allow),
  }
}

impl_pre_gate!(PrSize, "pr-size", evaluate);

#[cfg(test)]
#[path = "tests/pr_size_test.rs"]
mod tests;
