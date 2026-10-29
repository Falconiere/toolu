//! Deferred quality decisions over one shared scan of edited paths.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use toolu_protocol::decision::Decision;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::edit_records::EditRecord;
use toolu_state::gate_file::clear_gate_file;

use super::{
  AstGrepHit, AstGrepScan, EditedFile, QualityBinding, QualityRule, batch_scan, edited_record,
  file_scan, owns, partition_hits, selected_facts, settle,
};

struct PreparedFile {
  file: EditedFile,
  linked: bool,
  gate: PathBuf,
}

struct Prepared {
  files: Vec<Option<PreparedFile>>,
  enabled: Vec<usize>,
  scan: AstGrepScan,
  by_path: HashMap<String, Vec<AstGrepHit>>,
  state: StateCtx,
}

/// One event's scan, with decisions settled only when their manifests are reached.
pub(crate) struct QualityBatch {
  records: Vec<EditRecord>,
  prepared: Option<Prepared>,
}

impl QualityBatch {
  /// Capture the ordered paths without scanning or writing gate state.
  pub(crate) fn new(records: &[EditRecord]) -> Self {
    Self {
      records: records.to_vec(),
      prepared: None,
    }
  }

  fn prepare(
    &self,
    active: &[usize],
    bindings: &[QualityBinding<'_>],
    ctx: &RuleContext<'_>,
  ) -> Prepared {
    let enabled: Vec<usize> = active
      .iter()
      .copied()
      .filter(|index| {
        bindings
          .get(*index)
          .is_some_and(|binding| binding.rule.project_enabled(ctx))
      })
      .collect();
    let rules: Vec<&dyn QualityRule> = enabled
      .iter()
      .filter_map(|index| bindings.get(*index).map(|binding| binding.rule))
      .collect();
    let state = StateCtx::new(Roots::new(ctx.env.clone(), Some(ctx.host)));
    let files: Vec<Option<PreparedFile>> = self
      .records
      .iter()
      .map(|record| {
        let file = edited_record(record, ctx)?;
        let facts = selected_facts(std::slice::from_ref(&file), &rules, ctx, &state.roots);
        facts.first().map(|facts| PreparedFile {
          file: file.clone(),
          linked: facts.linked,
          gate: facts.gate.clone(),
        })
      })
      .collect();
    let mut seen = HashSet::new();
    let live: Vec<EditedFile> = files
      .iter()
      .flatten()
      .filter(|facts| {
        !facts.file.removed
          && seen.insert(facts.file.path.clone())
          && rules
            .iter()
            .any(|rule| owns(*rule, &facts.file) && !(rule.skip_linked_worktrees() && facts.linked))
      })
      .map(|facts| facts.file.clone())
      .collect();
    let mut scan = batch_scan(&rules, &live, ctx);
    let by_path = partition_hits(&mut scan);
    Prepared {
      files,
      enabled,
      scan,
      by_path,
      state,
    }
  }

  /// Evaluate one reached manifest against the event scan and settle its gate entry.
  pub(crate) fn decide(
    &mut self,
    record: usize,
    (binding, active, bindings): (usize, &[usize], &[QualityBinding<'_>]),
    ctx: &RuleContext<'_>,
  ) -> (Decision, Vec<String>) {
    if self.prepared.is_none() {
      self.prepared = Some(self.prepare(active, bindings, ctx));
    }
    let Some(prepared) = self.prepared.as_mut() else {
      return (Decision::Allow, Vec::new());
    };
    let Some(rule) = bindings.get(binding).map(|binding| binding.rule) else {
      return (Decision::Allow, Vec::new());
    };
    let decision = if !prepared.enabled.contains(&binding) {
      Decision::Allow
    } else if let Some(facts) = prepared.files.get(record).and_then(Option::as_ref) {
      if !owns(rule, &facts.file) {
        Decision::Allow
      } else if facts.file.removed {
        clear_gate_file(
          &mut prepared.state,
          &facts.gate,
          &facts.file.path,
          rule.source(),
        );
        Decision::Allow
      } else if rule.skip_linked_worktrees() && facts.linked {
        Decision::Allow
      } else {
        let scan = file_scan(&prepared.scan, &prepared.by_path, &facts.file);
        let findings = rule.check(&facts.file, ctx, &scan);
        settle(
          &facts.file,
          rule,
          &facts.gate,
          &mut prepared.state,
          findings,
        )
      }
    } else {
      Decision::Allow
    };
    (decision, std::mem::take(&mut prepared.state.warnings))
  }
}

#[cfg(test)]
#[path = "../tests/run_batch_test.rs"]
mod tests;
