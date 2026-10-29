//! Deferred quality decisions over one shared scan of edited paths.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use toolu_protocol::decision::Decision;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::edit_records::EditRecord;
use toolu_state::gate_file::clear_gate_file;

use super::{
  AstGrepHit, AstGrepScan, EditedFile, QualityBinding, QualityRule, edited_record, file_scan, owns,
  selected_facts, settle,
};

mod scan;
use scan::retry_file;
pub(super) use scan::scan_batch_with_fallback;

struct PreparedFile {
  file: EditedFile,
  linked: bool,
  gate: PathBuf,
}

struct Prepared {
  files: Vec<Option<PreparedFile>>,
  active: Vec<usize>,
  enabled: Vec<usize>,
  scan: AstGrepScan,
  by_path: HashMap<String, Vec<AstGrepHit>>,
  fallback: HashMap<String, AstGrepScan>,
  fingerprints: HashMap<String, Option<u64>>,
  checked: HashSet<usize>,
  rescanned: HashMap<String, AstGrepScan>,
  state: StateCtx,
}

fn enabled_bindings(
  active: &[usize],
  bindings: &[QualityBinding<'_>],
  ctx: &RuleContext<'_>,
) -> Vec<usize> {
  active
    .iter()
    .copied()
    .filter(|index| {
      bindings
        .get(*index)
        .is_some_and(|binding| binding.rule.project_enabled(ctx))
    })
    .collect()
}

fn fingerprint(path: &Path) -> Option<u64> {
  let bytes = std::fs::read(path).ok()?;
  let mut hash = DefaultHasher::new();
  bytes.hash(&mut hash);
  Some(hash.finish())
}

fn selected_file(
  record: &EditRecord,
  rules: &[&dyn QualityRule],
  ctx: &RuleContext<'_>,
  roots: &Roots,
) -> Option<PreparedFile> {
  let file = edited_record(record, ctx)?;
  let facts = selected_facts(std::slice::from_ref(&file), rules, ctx, roots);
  facts.first().map(|facts| PreparedFile {
    file: file.clone(),
    linked: facts.linked,
    gate: facts.gate.clone(),
  })
}

fn selected_files(
  records: &[EditRecord],
  rules: &[&dyn QualityRule],
  ctx: &RuleContext<'_>,
  roots: &Roots,
) -> Vec<Option<PreparedFile>> {
  records
    .iter()
    .map(|record| selected_file(record, rules, ctx, roots))
    .collect()
}

impl Prepared {
  fn refresh(
    &mut self,
    record: usize,
    edit: &EditRecord,
    bindings: &[QualityBinding<'_>],
    ctx: &RuleContext<'_>,
  ) {
    let enabled = enabled_bindings(&self.active, bindings, ctx);
    if enabled != self.enabled {
      self.enabled = enabled;
      self.fingerprints.clear();
      self.fallback.clear();
      self.rescanned.clear();
    }
    let rules: Vec<&dyn QualityRule> = self
      .enabled
      .iter()
      .filter_map(|index| bindings.get(*index).map(|binding| binding.rule))
      .collect();
    let Some(slot) = self.files.get_mut(record) else {
      return;
    };
    *slot = selected_file(edit, &rules, ctx, &self.state.roots);
    let Some(facts) = slot.as_ref() else {
      return;
    };
    if facts.file.removed
      || !rules
        .iter()
        .any(|rule| owns(*rule, &facts.file) && !(rule.skip_linked_worktrees() && facts.linked))
    {
      return;
    }
    let path = facts.file.path.clone();
    let current = fingerprint(&facts.file.absolute);
    if self.fingerprints.get(&path).copied() == Some(current) {
      return;
    }
    let scan = retry_file(&facts.file, &rules, ctx);
    self.rescanned.insert(path.clone(), scan);
    self.fingerprints.insert(path, current);
  }

  fn decide(
    &mut self,
    record: usize,
    binding: usize,
    rule: &dyn QualityRule,
    ctx: &RuleContext<'_>,
  ) -> Decision {
    if !self.enabled.contains(&binding) {
      return Decision::Allow;
    }
    let Some(facts) = self.files.get(record).and_then(Option::as_ref) else {
      return Decision::Allow;
    };
    if !owns(rule, &facts.file) {
      Decision::Allow
    } else if facts.file.removed {
      clear_gate_file(
        &mut self.state,
        &facts.gate,
        &facts.file.path,
        rule.source(),
      );
      Decision::Allow
    } else if rule.skip_linked_worktrees() && facts.linked {
      Decision::Allow
    } else {
      let scan = self
        .rescanned
        .get(&facts.file.path)
        .or_else(|| self.fallback.get(&facts.file.path))
        .cloned()
        .unwrap_or_else(|| file_scan(&self.scan, &self.by_path, &facts.file));
      let findings = rule.check(&facts.file, ctx, &scan);
      settle(&facts.file, rule, &facts.gate, &mut self.state, findings)
    }
  }
}

/// One event's scan, with decisions settled only when their manifests are reached.
pub(crate) struct QualityBatch {
  records: Vec<EditRecord>,
  prepared: Option<Prepared>,
  invalidated: bool,
}

impl QualityBatch {
  /// Capture the ordered paths without scanning or writing gate state.
  pub(crate) fn new(records: &[EditRecord]) -> Self {
    Self {
      records: records.to_vec(),
      prepared: None,
      invalidated: false,
    }
  }

  /// Executables may change files that the first quality manifest already scanned.
  pub(crate) fn invalidate(&mut self) {
    if let Some(prepared) = self.prepared.as_mut() {
      prepared.checked.clear();
      self.invalidated = true;
    }
  }

  fn prepare(
    &self,
    active: &[usize],
    bindings: &[QualityBinding<'_>],
    ctx: &RuleContext<'_>,
  ) -> Prepared {
    let enabled = enabled_bindings(active, bindings, ctx);
    let rules: Vec<&dyn QualityRule> = enabled
      .iter()
      .filter_map(|index| bindings.get(*index).map(|binding| binding.rule))
      .collect();
    let state = StateCtx::new(Roots::new(ctx.env.clone(), Some(ctx.host)));
    let files = selected_files(&self.records, &rules, ctx, &state.roots);
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
    let (scan, by_path, fallback) = scan_batch_with_fallback(&rules, &live, ctx);
    let fingerprints = live
      .iter()
      .map(|file| (file.path.clone(), fingerprint(&file.absolute)))
      .collect();
    Prepared {
      files,
      active: active.to_vec(),
      enabled,
      scan,
      by_path,
      fallback,
      fingerprints,
      checked: HashSet::new(),
      rescanned: HashMap::new(),
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
      self.invalidated = false;
    }
    let Some(prepared) = self.prepared.as_mut() else {
      return (Decision::Allow, Vec::new());
    };
    let Some(rule) = bindings.get(binding).map(|binding| binding.rule) else {
      return (Decision::Allow, Vec::new());
    };
    if self.invalidated
      && prepared.checked.insert(record)
      && let Some(edit) = self.records.get(record)
    {
      prepared.refresh(record, edit, bindings, ctx);
    }
    let decision = prepared.decide(record, binding, rule, ctx);
    (decision, std::mem::take(&mut prepared.state.warnings))
  }
}

#[cfg(test)]
#[path = "tests/batch_test.rs"]
mod tests;
