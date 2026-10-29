//! One post-edit batch: select owners, scan once, then settle their gate entries.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::git::{Discovery, discover, toplevel};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::edit_records::EditRecord;
use toolu_state::gate_file::{GateFailure, clear_gate_file, record_gate_failure};

use super::edit::edited_record;
use super::scan::{partition_hits, scan_rules};
use super::{
  AstGrepHit, AstGrepScan, EditedFile, edited_file, in_linked_worktree, is_regular_file,
};

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
  /// Structural rule YAML compiled into the rule crate, joined with the
  /// directories' rules into the event's one scan; empty adds nothing.
  fn ast_rules_inline(&self, _ctx: &RuleContext<'_>) -> String {
    String::new()
  }
  /// Check one file with the batch scan restricted to that file.
  fn check(&self, file: &EditedFile, ctx: &RuleContext<'_>, scan: &AstGrepScan) -> QualityFindings;
}

/// A compiled quality rule paired with the manifest that enables it.
pub struct QualityBinding<'a> {
  /// Registry spec.
  pub spec: &'a str,
  /// Registry rule name.
  pub name: &'a str,
  /// Rule used by the shared scan.
  pub rule: &'a dyn QualityRule,
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

/// Repository facts for one selected file, valid only during this run.
struct FileFacts<'a> {
  file: &'a EditedFile,
  linked: bool,
  gate: PathBuf,
}

fn gate_path(root: &Path, roots: &Roots) -> PathBuf {
  roots
    .project_state_root(None, Some(root))
    .unwrap_or_else(|| root.join(roots.project_dirname()).join("tmp"))
    .join("quality-gate-status.json")
}

fn file_facts<'a>(
  file: &'a EditedFile,
  ctx: &RuleContext<'_>,
  roots: &Roots,
  check_linked: bool,
  all_skip_linked: bool,
) -> Option<FileFacts<'a>> {
  if !file.removed && !is_regular_file(file) {
    return None;
  }
  let parent = file.absolute.parent().unwrap_or(&file.absolute);
  let (root, linked) = match discover(ctx.env, parent) {
    Discovery::Repo(repo) => {
      let linked = repo.git_dir != repo.common_dir;
      if linked && all_skip_linked {
        return None;
      }
      (
        repo
          .toplevel
          .unwrap_or_else(|| ctx.project_root.to_path_buf()),
        linked,
      )
    }
    Discovery::NotFound => (ctx.project_root.to_path_buf(), false),
    Discovery::AskGit => {
      let linked = check_linked && in_linked_worktree(file, ctx);
      if linked && all_skip_linked {
        return None;
      }
      (
        toplevel(ctx.env, parent).unwrap_or_else(|| ctx.project_root.to_path_buf()),
        linked,
      )
    }
  };
  Some(FileFacts {
    file,
    linked,
    gate: gate_path(&root, roots),
  })
}

fn selected_facts<'a>(
  files: &'a [EditedFile],
  enabled: &[&dyn QualityRule],
  ctx: &RuleContext<'_>,
  roots: &Roots,
) -> Vec<FileFacts<'a>> {
  files
    .iter()
    .filter_map(|file| {
      let mut owned = false;
      let mut check_linked = false;
      let mut all_skip_linked = true;
      for rule in enabled {
        if owns(*rule, file) {
          owned = true;
          let skips = rule.skip_linked_worktrees();
          check_linked |= skips;
          all_skip_linked &= skips;
        }
      }
      if !owned {
        return None;
      }
      check_linked &= !file.removed;
      all_skip_linked &= check_linked;
      file_facts(file, ctx, roots, check_linked, all_skip_linked)
    })
    .collect()
}

fn live_files(facts: &[FileFacts<'_>], enabled: &[&dyn QualityRule]) -> Vec<EditedFile> {
  facts
    .iter()
    .filter(|facts| {
      !facts.file.removed
        && enabled
          .iter()
          .any(|rule| owns(*rule, facts.file) && !(rule.skip_linked_worktrees() && facts.linked))
    })
    .map(|facts| facts.file.clone())
    .collect()
}

fn settle(
  file: &EditedFile,
  rule: &dyn QualityRule,
  gate: &Path,
  state: &mut StateCtx,
  findings: QualityFindings,
) -> Decision {
  if findings.errors.is_empty() {
    clear_gate_file(state, gate, &file.path, rule.source());
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
    gate,
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

fn file_scan(
  scan: &AstGrepScan,
  by_path: &HashMap<String, Vec<AstGrepHit>>,
  file: &EditedFile,
) -> AstGrepScan {
  match scan {
    AstGrepScan::Ok { empty, .. } => AstGrepScan::Ok {
      hits: by_path.get(&file.path).cloned().unwrap_or_default(),
      empty: *empty,
    },
    other @ (AstGrepScan::Missing | AstGrepScan::Failed(_)) => other.clone(),
  }
}

/// One ast-grep scan of `live` with every enabled rule's YAML, directories and inline.
fn batch_scan(
  enabled: &[&dyn QualityRule],
  live: &[EditedFile],
  ctx: &RuleContext<'_>,
) -> AstGrepScan {
  let dirs: Vec<PathBuf> = enabled
    .iter()
    .flat_map(|rule| rule.ast_rule_dirs())
    .collect();
  let paths: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
  let inline: Vec<String> = enabled
    .iter()
    .map(|rule| rule.ast_rules_inline(ctx))
    .filter(|rules| !rules.is_empty())
    .collect();
  scan_rules(live, &paths, &inline, ctx)
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
  let mut state = StateCtx::new(Roots::new(ctx.env.clone(), Some(ctx.host)));
  let facts = selected_facts(files, &enabled, ctx, &state.roots);
  let live = live_files(&facts, &enabled);
  let mut scan = batch_scan(&enabled, &live, ctx);
  let by_path = partition_hits(&mut scan);
  let mut decisions = Vec::new();
  for facts in facts {
    let file = facts.file;
    for rule in &enabled {
      if !owns(*rule, file) {
        continue;
      }
      if file.removed {
        clear_gate_file(&mut state, &facts.gate, &file.path, rule.source());
      } else if !(rule.skip_linked_worktrees() && facts.linked) {
        let findings = rule.check(file, ctx, &file_scan(&scan, &by_path, file));
        decisions.push(settle(file, *rule, &facts.gate, &mut state, findings));
      }
    }
  }
  QualityOutcome {
    decisions,
    warnings: state.warnings,
  }
}

#[path = "run/batch.rs"]
mod batch;

pub(crate) use batch::QualityBatch;

/// Last occurrence of each exact `path`, in last-occurrence order.
fn last_edit_records(records: &[EditRecord]) -> Vec<&EditRecord> {
  let mut probes = 0_u64;
  last_edit_records_probed(records, &mut probes)
}

/// Same selection as [`last_edit_records`], counting one membership probe per record.
fn last_edit_records_probed<'a>(
  records: &'a [EditRecord],
  probes: &mut u64,
) -> Vec<&'a EditRecord> {
  let mut seen = HashSet::new();
  let mut survivors = Vec::new();
  for record in records.iter().rev() {
    *probes += 1;
    if seen.insert(record.path.as_str()) {
      survivors.push(record);
    }
  }
  survivors.reverse();
  survivors
}

/// Run all enabled quality rules over normalized edit records of one tool call.
pub fn run_quality(
  records: &[EditRecord],
  ctx: &RuleContext<'_>,
  rules: &[&dyn QualityRule],
) -> QualityOutcome {
  let files: Vec<EditedFile> = last_edit_records(records)
    .into_iter()
    .filter_map(|record| edited_record(record, ctx))
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

#[cfg(test)]
#[path = "tests/run_facts_test.rs"]
mod facts_tests;

#[cfg(test)]
#[path = "tests/run_edit_test.rs"]
mod edit_tests;
