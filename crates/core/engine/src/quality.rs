//! Shared post-edit quality flow for language rule crates (#459).

mod edit;
mod manifest_rule;
mod plugin;
mod run;
mod scan;

pub use edit::{EditedFile, edited_file, in_linked_worktree, is_regular_file};
pub use manifest_rule::ManifestRule;
pub use plugin::{QualityPlugin, check_toolu, publish};
pub use run::{QualityFindings, QualityOutcome, QualityRule, run_quality, run_quality_event};
pub use scan::{
  AstGrepHit, AstGrepScan, ScanFailure, ScanStage, partition_hits, scan_inline, scan_rule_dirs,
};
