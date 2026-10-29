//! Structural scans shared by one quality batch, with per-file overflow recovery.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use toolu_runtime::registry::rule::RuleContext;

use super::super::super::scan::{output_limited, partition_hits, scan_rules};
use super::super::{AstGrepHit, AstGrepScan, EditedFile, QualityRule};

/// One ast-grep scan of `live` with every enabled rule's YAML, directories and inline.
pub(super) fn batch_scan(
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

/// Keep normal batches in one scan; isolate files only when their combined output overflows.
pub(in crate::quality::run) fn scan_batch_with_fallback(
  enabled: &[&dyn QualityRule],
  live: &[EditedFile],
  ctx: &RuleContext<'_>,
) -> (
  AstGrepScan,
  HashMap<String, Vec<AstGrepHit>>,
  HashMap<String, AstGrepScan>,
) {
  let mut scan = batch_scan(enabled, live, ctx);
  if output_limited(&scan) && live.len() > 1 {
    let per_file = live
      .iter()
      .map(|file| {
        (
          file.path.clone(),
          batch_scan(enabled, std::slice::from_ref(file), ctx),
        )
      })
      .collect();
    return (scan, HashMap::new(), per_file);
  }
  let by_path = partition_hits(&mut scan);
  (scan, by_path, HashMap::new())
}
