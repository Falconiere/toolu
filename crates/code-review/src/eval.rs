//! The expectation-based eval harness (#670): which known defects a review caught,
//! which known false positives it raised, and what it left for a human to classify.
//! `expectations` is the committed case schema, `matcher` pairs findings with
//! expectation rows by exact path and a case-insensitive claim substring, and
//! `report` is the paired-comparison document with each call's `live` or `replayed`
//! origin, usage, cost, runtime and tokens-per-finding. Nothing here touches the
//! network; the runner and CLI are layered on top of these types. `fixtures` loads the
//! committed golden set and materializes each case into a real Git repository, and
//! `gate` checks the gate contract with the production blocking predicates.

/// The committed case set: classes, expectation rows, memory seeds, gate flags.
pub mod expectations;
/// The golden set on disk and its materialized Git repositories.
pub mod fixtures;
/// The offline gate-contract checks.
pub mod gate;
/// Pairing findings with expectations.
pub mod matcher;
/// The machine-readable paired-comparison report.
pub mod report;

pub use expectations::{Case, CaseSet, Class, Expectation, Gate, Kind};
pub use fixtures::{DEFAULT_BASE, Fixtures};
pub use gate::violations;
pub use matcher::{Match, Outcome, Status, Tally, match_findings};
pub use report::{
  Arm, ArmReport, CallRecord, EvalReport, FindingsSummary, LiveOrReplayed, Revisions, Run,
  UsageTotals,
};
