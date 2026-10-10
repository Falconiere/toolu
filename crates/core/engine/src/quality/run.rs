//! One post-edit batch: select owners, scan once, then settle their gate entries.

use std::path::{Path, PathBuf};

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::edit_records::EditRecord;
use toolu_state::gate_file::{GateFailure, clear_gate_file, record_gate_failure};

use super::edit::edited_record;
use super::scan::scan_rule_dirs;
use super::{AstGrepScan, EditedFile, edited_file, in_linked_worktree, is_regular_file};

/// Violations and nonblocking advice in check order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QualityFindings {
  /// Each violation becomes one newline-terminated gate line.
  pub errors: Vec<String>,
  /// Empty advisories are discarded before joining.
  pub advisories: Vec<String>,
}

/// A language rule hosted by the shared quality flow.
pub trait QualityRule: Send + Sync {
  /// Language named by this rule, such as `ts`, `python` or `rust`.
  fn language(&self) -> &str;
  /// Extensions, without dots and with exact case.
  fn extensions(&self) -> &[&str];
  /// The source that owns this rule's gate entries.
  fn source(&self) -> &str;
  /// The reason recorded beside a violation.
  fn reason(&self) -> &str;
  /// Whether the project's configuration enables this rule.
  fn project_enabled(&self, ctx: &RuleContext<'_>) -> bool;
  /// Whether this rule skips files in linked Git worktrees.
  fn skip_linked_worktrees(&self) -> bool;
  /// Directories of structural rule YAML supplied by the rule crate.
  fn ast_rule_dirs(&self) -> Vec<PathBuf>;
  /// Check one file with the batch scan restricted to that file.
  fn check(&self, file: &EditedFile, ctx: &RuleContext<'_>, scan: &AstGrepScan) -> QualityFindings;
}

/// Ordered decisions and state warnings from one edit event.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QualityOutcome {
  /// A decision for each selected file and rule, in file then rule order.
  pub decisions: Vec<Decision>,
  /// Lock, schema, telemetry or write warnings to print to the caller.
  pub warnings: Vec<String>,
}

fn owns(rule: &dyn QualityRule, file: &EditedFile) -> bool {
  file
    .absolute
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| rule.extensions().contains(&extension))
}

fn gate_path(file: &EditedFile, ctx: &RuleContext<'_>, state: &StateCtx) -> PathBuf {
  let parent = file.absolute.parent().unwrap_or(&file.absolute);
  let root = toplevel(ctx.env, parent).unwrap_or_else(|| ctx.project_root.to_path_buf());
  state
    .roots
    .project_state_root(None, Some(&root))
    .unwrap_or_else(|| root.join(state.roots.project_dirname()).join("tmp"))
    .join("quality-gate-status.json")
}

fn settle(
  file: &EditedFile,
  rule: &dyn QualityRule,
  ctx: &RuleContext<'_>,
  state: &mut StateCtx,
  findings: QualityFindings,
) -> Decision {
  let gate = gate_path(file, ctx, state);
  if findings.errors.is_empty() {
    clear_gate_file(state, &gate, &file.path, rule.source());
    let advice = findings
      .advisories
      .into_iter()
      .filter(|text| !text.is_empty())
      .collect::<Vec<_>>()
      .join("\n");
    return Text::new(advice).map_or(Decision::Allow, |message| Decision::Advisory { message });
  }
  let violations = findings
    .errors
    .into_iter()
    .fold(String::new(), |mut text, error| {
      text.push_str(&error);
      text.push('\n');
      text
    });
  if let Some(parent) = gate.parent()
    && let Err(error) = std::fs::create_dir_all(parent)
  {
    state.warnings.push(format!(
      "quality: cannot create {}: {error}",
      parent.display()
    ));
  }
  record_gate_failure(
    state,
    &gate,
    &GateFailure {
      file: &file.path,
      source: rule.source(),
      reason: rule.reason(),
      violations: &violations,
    },
  );
  let message = format!("QUALITY VIOLATION — fix before proceeding:\n{violations}");
  Text::new(message).map_or(Decision::Allow, |message| Decision::Advisory { message })
}

fn file_scan(scan: &AstGrepScan, file: &EditedFile) -> AstGrepScan {
  match scan {
    AstGrepScan::Ok { hits, empty } => AstGrepScan::Ok {
      hits: hits
        .iter()
        .filter(|hit| hit.file == file.path)
        .cloned()
        .collect(),
      empty: *empty,
    },
    other @ (AstGrepScan::Missing | AstGrepScan::Failed(_)) => other.clone(),
  }
}

fn run_files(
  files: &[EditedFile],
  ctx: &RuleContext<'_>,
  rules: &[&dyn QualityRule],
) -> QualityOutcome {
  let enabled: Vec<&dyn QualityRule> = rules
    .iter()
    .copied()
    .filter(|rule| rule.project_enabled(ctx))
    .collect();
  let live: Vec<EditedFile> = files
    .iter()
    .filter(|file| !file.removed && is_regular_file(file))
    .filter(|file| {
      enabled.iter().any(|rule| {
        owns(*rule, file) && !(rule.skip_linked_worktrees() && in_linked_worktree(file, ctx))
      })
    })
    .cloned()
    .collect();
  let dirs: Vec<PathBuf> = enabled
    .iter()
    .flat_map(|rule| rule.ast_rule_dirs())
    .collect();
  let paths: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
  let scan = scan_rule_dirs(&live, &paths, ctx);
  let mut state = StateCtx::new(Roots::new(ctx.env.clone(), Some(ctx.host)));
  let mut decisions = Vec::new();
  for file in files {
    for rule in &enabled {
      if !owns(*rule, file) {
        continue;
      }
      if file.removed {
        let gate = gate_path(file, ctx, &state);
        clear_gate_file(&mut state, &gate, &file.path, rule.source());
      } else if is_regular_file(file)
        && !(rule.skip_linked_worktrees() && in_linked_worktree(file, ctx))
      {
        let findings = rule.check(file, ctx, &file_scan(&scan, file));
        decisions.push(settle(file, *rule, ctx, &mut state, findings));
      }
    }
  }
  QualityOutcome {
    decisions,
    warnings: state.warnings,
  }
}

/// Run all enabled quality rules over normalized edit records of one tool call.
pub fn run_quality(
  records: &[EditRecord],
  ctx: &RuleContext<'_>,
  rules: &[&dyn QualityRule],
) -> QualityOutcome {
  let files: Vec<EditedFile> = records
    .iter()
    .enumerate()
    .filter(|(index, record)| {
      !records
        .iter()
        .skip(index + 1)
        .any(|next| next.path == record.path)
    })
    .filter_map(|(_, record)| edited_record(record, ctx))
    .collect();
  run_files(&files, ctx, rules)
}

/// Run the legacy single-file edit path over one normalized post-tool event.
pub fn run_quality_event(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  rules: &[&dyn QualityRule],
) -> QualityOutcome {
  let files: Vec<EditedFile> = edited_file(event, ctx).into_iter().collect();
  run_files(&files, ctx, rules)
}

#[cfg(test)]
#[path = "tests/run_test.rs"]
mod tests;
