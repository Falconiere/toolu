//! Compact agent-facing rendering; the structured JSON stays the source of truth.

use std::cmp::Reverse;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};

use crate::contract::Report;

pub(crate) fn present(report: &Report, ctx: &Ctx) -> Outcome {
  let stdout = if ctx.json {
    match serde_json::to_string(report) {
      Ok(doc) => doc,
      Err(err) => return Outcome::failed(Exit::Failure, format!("code-review: {err}")),
    }
  } else {
    summary(report)
  };
  Outcome {
    exit: report.exit(),
    stdout: Some(stdout),
    stderr: None,
  }
}

fn summary(report: &Report) -> String {
  let source = report.effort_source.as_deref().unwrap_or("unspecified");
  let mut lines = vec![format!(
    "- [{}] code-review: {} · {} blocking · effort: {} ({source})",
    if report.exit() == Exit::Success {
      "x"
    } else {
      " "
    },
    report.verdict().name(),
    report.blocking_count(),
    report.effort.name()
  )];
  if let Some(error) = &report.error {
    lines.push(format!("{}: {}", error.kind.name(), error.message));
  }
  append_sections(report, &mut lines);
  let cost = report
    .usage
    .cost_usd
    .and_then(|cost| serde_json::to_string(&cost).ok())
    .unwrap_or_else(|| "unknown".to_owned());
  lines.push(format!(
    "Usage: {} in, {} out, ${cost}",
    report.usage.tokens_in, report.usage.tokens_out
  ));
  lines.join("\n")
}

fn append_sections(report: &Report, lines: &mut Vec<String>) {
  let mut findings: Vec<_> = report.findings.iter().collect();
  findings.sort_by_key(|finding| Reverse(finding.severity));
  if !findings.is_empty() {
    lines.push("Findings".to_owned());
    lines.extend(findings.into_iter().map(|finding| {
      format!(
        "[{}{}] {}:{} {}{}",
        finding.severity.name(),
        finding
          .assessment
          .map_or(String::new(), |assessment| format!(
            ", jev: {}",
            assessment.name()
          )),
        finding.path,
        finding.line,
        finding.claim,
        finding
          .suggestion
          .as_ref()
          .map_or(String::new(), |fix| format!(" → {fix}"))
      )
    }));
  }
  if !report.coverage.unreviewed.is_empty() {
    lines.push("Unreviewed".to_owned());
    lines.extend(
      report
        .coverage
        .unreviewed
        .iter()
        .map(|item| format!("{}: {}", item.path, item.reason)),
    );
  }
  if !report.recap.is_empty() {
    lines.push("Recap".to_owned());
    lines.extend(report.recap.iter().cloned());
  }
  if !report.mechanical_checks.is_empty() {
    lines.push("Mechanical checks".to_owned());
    lines.extend(report.mechanical_checks.iter().cloned());
  }
  if !report.notes.is_empty() {
    lines.push("Notes".to_owned());
    lines.extend(report.notes.iter().cloned());
  }
}

#[cfg(test)]
#[path = "tests/report_test.rs"]
mod tests;
