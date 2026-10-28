//! The `Mechanical checks` section body: what a review reports when no model could
//! judge the diff.

use std::cmp::Reverse;

use super::PrepassResult;
use crate::contract::Finding;

/// The line shown when the pre-pass found and noticed nothing.
const NOTHING: &str = "No mechanical findings.";

fn line(finding: &Finding) -> String {
  format!(
    "[{}/{}] {}:{} {}",
    finding.source.name(),
    finding.severity.name(),
    finding.path,
    finding.line,
    finding.claim
  )
}

/// The body lines of the `Mechanical checks` section: every finding worst first,
/// then every notice, or exactly `No mechanical findings.` when both are empty.
/// Never empty, and never a verdict: the report computes that from its findings.
pub fn mechanical_checks_lines(result: &PrepassResult) -> Vec<String> {
  let mut findings: Vec<&Finding> = result.findings.iter().collect();
  findings.sort_by_key(|finding| Reverse(finding.severity));
  let mut lines: Vec<String> = findings.into_iter().map(line).collect();
  lines.extend(result.notices.iter().map(|notice| notice.text.clone()));
  if lines.is_empty() {
    lines.push(NOTHING.to_owned());
  }
  lines
}

#[cfg(test)]
#[path = "tests/mechanical_test.rs"]
mod tests;
