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
/// and prose. Each path is also matched with a leading `/`, so `*/tests/*`
/// covers a root `tests/`. A `gates.prSize.exclude` array replaces the list.
const NOT_CODE: &[&str] = &[
  "*.lock",
  "*-lock.json",
  "*-lock.yaml",
  "*/go.sum",
  "*.snap",
  "*.min.js",
  "*.min.css",
  "*/tests/*",
  "*/test/*",
  "*/__tests__/*",
  "*/fixtures/*",
  "*/testdata/*",
  "*_test.*",
  "*.test.*",
  "*.spec.*",
  "*/test_*.py",
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
  let Some(value) = setting(config, key) else {
    return fallback;
  };
  value.as_u64().filter(|max| *max > 0).unwrap_or_else(|| {
    config.warn(format!(
      "gates.prSize.{key}: {value} is not a positive integer; using {fallback}"
    ));
    fallback
  })
}

fn globs(config: &LoadedConfig, key: &str) -> Option<Vec<String>> {
  let items = setting(config, key)?.as_array()?;
  let strings = items.iter().filter_map(Value::as_str);
  Some(strings.map(str::to_owned).collect())
}

/// The code lines and changed files of `git diff --numstat` output. A binary
/// file (`-`) or an excluded path is a changed file with no code lines.
fn counted(numstat: &str, config: &LoadedConfig) -> (u64, u64) {
  let mut excludes = globs(config, "exclude")
    .unwrap_or_else(|| NOT_CODE.iter().map(|glob| (*glob).to_owned()).collect());
  excludes.extend(globs(config, "alsoExclude").unwrap_or_default());
  let mut total = (0, 0);
  for line in numstat.lines() {
    let mut fields = line.splitn(3, '\t');
    let (Some(added), Some(path)) = (fields.next(), fields.nth(1)) else {
      continue;
    };
    total.1 += 1;
    if !matches_any(path, &excludes) && !matches_any(&format!("/{path}"), &excludes) {
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

/// The size of the checked-out branch: [`rev_size`] of `HEAD`.
///
/// # Errors
/// As [`rev_size`].
pub(crate) fn branch_size(
  root: &Path,
  env: &Env,
  config: &LoadedConfig,
) -> Result<Option<Size>, String> {
  rev_size(root, env, config, "HEAD")
}

/// The size of `rev`, or none when the base exists but Git cannot diff
/// against it, which push-review allows too (`diff-failed`).
///
/// # Errors
/// The base ref, when it is missing: an unmeasured branch is not a small one.
pub(crate) fn rev_size(
  root: &Path,
  env: &Env,
  config: &LoadedConfig,
  rev: &str,
) -> Result<Option<Size>, String> {
  let base = size_base(root, env, &base_branch(env, Some(root), root));
  if !ref_exists(root, &base, env) {
    return Err(base);
  }
  let range = format!("{base}...{rev}");
  let args = ["-c", "core.quotepath=false", "diff", "--numstat", &range];
  let Some(numstat) = git_at(root, &args, env) else {
    return Ok(None);
  };
  let (added, files) = counted(&numstat, config);
  Ok(Some(Size {
    added,
    max: limit(config, "maxLines", DEFAULT_MAX_LINES),
    files,
    max_files: limit(config, "maxFiles", DEFAULT_MAX_FILES),
    base,
  }))
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
  let size = branch_size(&root, ctx.env, &config);
  warnings.extend(config.take_warnings());
  match size {
    Ok(Some(size)) if size.over() => decided(mode, message(&size, mode)),
    Ok(_) => Ok(Decision::Allow),
    Err(base) => decided(
      mode,
      format!(
        "pr-size: cannot measure this branch: base '{base}' not found locally. Fetch it (`git fetch origin`), or set `PUSH_REVIEW_BASE` to the branch this one merges into, then push again."
      ),
    ),
  }
}

impl_pre_gate!(PrSize, "pr-size", evaluate);

#[cfg(test)]
#[path = "tests/pr_size_test.rs"]
mod tests;
