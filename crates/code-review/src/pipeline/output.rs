//! Compose validated findings, coverage, recap, and outage diagnostics.

use crate::budget::Effort;
use crate::contract::{Coverage, Finding, Omitted, Report, Severity, Side, Source};
use crate::memory;
use crate::prepass::{self, PrepassResult};
use crate::review::{self, RunError};
use crate::shape::Shaped;
use crate::validate::{self, Recap};
use std::collections::BTreeSet;

pub(super) fn completed(
  shaped: &Shaped,
  prepass: &PrepassResult,
  prior: &memory::BranchState,
  reviewed: review::Reviewed,
) -> (Report, Recap) {
  let models: Vec<_> = reviewed
    .findings
    .iter()
    .map(|item| item.finding.clone())
    .collect();
  let validated = validate::validate(&validate::Input {
    shaped,
    model: &models,
    deterministic: &prepass.findings,
    memory: &prior.memory,
    redactions: &prepass.redactions,
    unreviewed: &reviewed.unreviewed,
  });
  let coverage = coverage(shaped, prior, &validated, reviewed.unreviewed);
  let mut report = Report::reviewed(
    reviewed.effort.level,
    validated.findings,
    coverage,
    reviewed.usage,
  );
  report.effort_source = Some(format!(
    "{:?}: {}",
    reviewed.effort.source, reviewed.effort.reason
  ));
  report.recap = recap_lines(&validated.recap, &prior.passes);
  report.notes = reviewed.notes;
  if !prepass.findings.is_empty() || !prepass.notices.is_empty() {
    report.mechanical_checks = prepass::mechanical_checks_lines(prepass);
  }
  (report, validated.recap)
}

fn coverage(
  shaped: &Shaped,
  prior: &memory::BranchState,
  validated: &validate::Validated,
  lost: Vec<Omitted>,
) -> Coverage {
  let mut coverage = shaped.coverage.clone();
  coverage.unreviewed.extend(lost);
  coverage
    .unreviewed
    .extend(validated.unreviewed.iter().cloned());
  let raised: BTreeSet<&str> = validated
    .findings
    .iter()
    .map(|finding| finding.id.as_str())
    .collect();
  for id in &validated.recap.still_open {
    if !raised.contains(id.as_str()) {
      let path = prior
        .memory
        .prior
        .iter()
        .find(|finding| &finding.id == id)
        .map_or("(review)", |finding| finding.path.as_str());
      coverage.unreviewed.push(Omitted {
        path: path.to_owned(),
        reason: format!("prior finding {id} was not re-examined"),
      });
    }
  }
  let blocked: BTreeSet<&str> = coverage
    .unreviewed
    .iter()
    .map(|item| item.path.as_str())
    .collect();
  coverage
    .reviewed
    .retain(|path| !blocked.contains(path.as_str()));
  coverage
}

pub(super) fn failed(
  shaped: &Shaped,
  prepass: &PrepassResult,
  effort: Effort,
  err: &RunError,
) -> (Report, Recap) {
  let outage = matches!(err, RunError::Outage { .. } | RunError::Deadline { .. });
  let mut report = err.report(effort);
  report.coverage = shaped.coverage.clone();
  report
    .coverage
    .unreviewed
    .extend(shaped.coverage.reviewed.iter().map(|path| Omitted {
      path: path.clone(),
      reason: "model delivered no judgment".to_owned(),
    }));
  report.coverage.reviewed.clear();
  report.findings.clone_from(&prepass.findings);
  if outage {
    report.findings.push(Finding {
      id: "code-review:outage".to_owned(),
      path: "(review)".to_owned(),
      line: 1,
      end_line: 1,
      side: Side::New,
      severity: Severity::Blocker,
      category: "provider".to_owned(),
      source: Source::System,
      claim: "The review model delivered no judgment; this diff cannot pass review".to_owned(),
      quote: "provider unavailable".to_owned(),
      suggestion: None,
      redaction_spans: Vec::new(),
    });
  }
  report.mechanical_checks = prepass::mechanical_checks_lines(prepass);
  (report, Recap::default())
}

fn recap_lines(recap: &Recap, passes: &[memory::Pass]) -> Vec<String> {
  let mut lines = Vec::new();
  for (label, ids) in [
    ("resolved", &recap.resolved),
    ("still open", &recap.still_open),
    ("new", &recap.new),
  ] {
    if !ids.is_empty() {
      lines.push(format!("{label}: {}", ids.join(", ")));
    }
  }
  for pass in passes.iter().rev().take(3) {
    lines.push(format!(
      "prior {}: {} ({} blocking, {} advisory)",
      pass.diff_sha,
      pass.verdict.name(),
      pass.counts.blocking,
      pass.counts.advisory
    ));
  }
  lines
}

#[cfg(test)]
#[path = "tests/output_test.rs"]
mod tests;
