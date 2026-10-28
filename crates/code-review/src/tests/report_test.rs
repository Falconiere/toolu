use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{present, summary};
use crate::budget::Effort;
use crate::contract::{Coverage, Omitted, Report, Usage};

#[test]
fn compact_summary_omits_empty_sections_and_names_unreviewed() {
  let report = Report::reviewed(
    Effort::Small,
    Vec::new(),
    Coverage::default(),
    Usage::default(),
  );
  let text = summary(&report);
  assert!(text.starts_with("- [x] code-review: approved · 0 blocking · effort: small"));
  assert!(!text.contains("Findings") && !text.contains("Mechanical checks"));
  let mut partial = report;
  partial.coverage.unreviewed.push(Omitted {
    path: "src/big.rs".into(),
    reason: "over budget".into(),
  });
  let outcome = present(&partial, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Blocked);
  assert!(outcome.stdout.unwrap().contains("src/big.rs: over budget"));
}
