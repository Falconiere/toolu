//! The gate contract a case asserts offline, checked against synthetic reports with
//! the production predicates: `Severity::blocking`, `Report::blocking_count` and
//! `verdict`, and the pipeline's `settle`, which decides whether a review may leave
//! a passing state. Nothing here reimplements a blocking rule.

use super::expectations::Gate;
use crate::budget::Effort;
use crate::contract::{Coverage, Finding, Omitted, Report, Severity, Side, Source, Usage, Verdict};

type Check = fn() -> Result<(), String>;

const SEVERITIES: [Severity; 5] = [
  Severity::Blocker,
  Severity::High,
  Severity::Medium,
  Severity::Low,
  Severity::Nit,
];

fn finding(severity: Severity) -> Finding {
  Finding {
    id: format!("gate-{}", severity.name()),
    path: "src/lib.rs".to_owned(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity,
    category: "correctness".to_owned(),
    source: Source::Llm,
    claim: format!("a {} finding", severity.name()),
    quote: "quote".to_owned(),
    suggestion: None,
    redaction_spans: Vec::new(),
    assessment: None,
  }
}

fn report(severities: &[Severity], unreviewed: Vec<Omitted>) -> Report {
  Report::reviewed(
    Effort::Medium,
    severities.iter().copied().map(finding).collect(),
    Coverage {
      reviewed: vec!["src/lib.rs".to_owned()],
      unreviewed,
      skipped: Vec::new(),
    },
    Usage::default(),
  )
}

fn omitted() -> Vec<Omitted> {
  vec![Omitted {
    path: "src/lib.rs".to_owned(),
    reason: "left unreviewed".to_owned(),
  }]
}

/// Blocking severities drive the blocking count: each severity alone counts exactly
/// when `Severity::blocking` says it does, and the whole set counts the blocking ones.
fn blocking_drives_count() -> Result<(), String> {
  for severity in SEVERITIES {
    let count = report(&[severity], Vec::new()).blocking_count();
    if count != usize::from(severity.blocking()) {
      return Err(format!(
        "a lone {} finding gave blocking count {count}",
        severity.name()
      ));
    }
  }
  let all = report(&SEVERITIES, Vec::new()).blocking_count();
  let blocking = SEVERITIES.iter().filter(|each| each.blocking()).count();
  if all == blocking && blocking > 0 && blocking < SEVERITIES.len() {
    Ok(())
  } else {
    Err(format!("mixed severities gave blocking count {all}"))
  }
}

/// Advisory findings never block: with full coverage the report is approved.
fn advisory_never_blocks() -> Result<(), String> {
  let advisory: Vec<Severity> = SEVERITIES
    .into_iter()
    .filter(|severity| !severity.blocking())
    .collect();
  let report = report(&advisory, Vec::new());
  if report.blocking_count() == 0 && report.verdict() == Verdict::Approved {
    Ok(())
  } else {
    Err(format!(
      "advisory findings gave {} blocking and verdict {}",
      report.blocking_count(),
      report.verdict().name()
    ))
  }
}

/// An unreviewed path never writes a passing state: `settle` reports the review
/// incomplete and leaves a blocking finding, so the gate count is above zero; a
/// complete review gains no blocker.
fn unreviewed_never_writes_pass() -> Result<(), String> {
  let mut partial = report(&[], omitted());
  let complete = crate::pipeline::settle(&mut partial);
  if complete || partial.blocking_count() == 0 || partial.verdict() == Verdict::Approved {
    return Err("an unreviewed path left a passing state".to_owned());
  }
  let mut whole = report(&[], Vec::new());
  if crate::pipeline::settle(&mut whole) && whole.blocking_count() == 0 {
    Ok(())
  } else {
    Err("a complete review was blocked".to_owned())
  }
}

/// Every violated flag of `gate`, one line each; empty when the contract holds.
pub fn violations(gate: &Gate) -> Vec<String> {
  let checks: [(bool, &str, Check); 3] = [
    (
      gate.blocking_severities_drive_count,
      "blocking_severities_drive_count",
      blocking_drives_count,
    ),
    (
      gate.advisory_never_blocks,
      "advisory_never_blocks",
      advisory_never_blocks,
    ),
    (
      gate.unreviewed_never_writes_pass,
      "unreviewed_never_writes_pass",
      unreviewed_never_writes_pass,
    ),
  ];
  checks
    .into_iter()
    .filter(|(enabled, _, _)| *enabled)
    .filter_map(|(_, name, check)| check().err().map(|why| format!("{name}: {why}")))
    .collect()
}

#[cfg(test)]
#[path = "tests/gate_test.rs"]
mod tests;
