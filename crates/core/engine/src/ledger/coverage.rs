//! The report-only AC-coverage section of `status` (`pl_ac_coverage_lines` in
//! `ledger-model.ts`): which fresh-green steps cover each spec `AC-<n>`.

use std::path::Path;

use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use super::jq::{JqError, alt, each, get, holds, is_str, type_name};
use super::model::status_is;
use super::parse::{is_specless, parse_acs};

/// One AC's coverage, for `--json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcCoverage {
  /// The `AC-<n>` id.
  pub id: String,
  /// Whether a fresh-green step covers it.
  pub covered: bool,
  /// The ids of the steps that reference it, as `join(", ")` renders each.
  pub steps: Vec<String>,
}

/// The AC-coverage report: stdout text, stderr lines and the structured rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoverageReport {
  /// `AC coverage (report-only):` and one line per AC.
  pub stdout: String,
  /// At most one tagged line, when the ledger cannot be read.
  pub stderr: Vec<String>,
  /// One row per line printed.
  pub rows: Vec<AcCoverage>,
}

/// jq `join(", ")` over step ids: null joins as empty, scalars as their text.
fn id_texts(ids: &[&Ordered]) -> Result<Vec<String>, JqError> {
  ids
    .iter()
    .map(|id| match id {
      Ordered::Null => Ok(String::new()),
      Ordered::String(text) => Ok(text.clone()),
      Ordered::Bool(_) | Ordered::Number(_) => Ok(jq_text(id, false)),
      Ordered::Array(_) | Ordered::Object(_) => {
        Err(JqError(format!("Cannot join with {}", type_name(id))))
      }
    })
    .collect()
}

/// One `pl_ac_coverage_lines` line for `ac`, and its row.
///
/// # Errors
/// [`JqError`] where jq fails on the ledger's shape.
pub fn ac_coverage_line(
  ledger: &Ordered,
  cur: &str,
  ac: &str,
) -> Result<(String, AcCoverage), JqError> {
  let empty = Ordered::Array(Vec::new());
  let mut covering = Vec::new();
  for step in each(get(ledger, "steps")?)? {
    if holds(alt(get(step, "ac_refs")?, &empty), ac)? {
      covering.push(step);
    }
  }
  let ids: Vec<&Ordered> = covering
    .iter()
    .map(|step| get(step, "id"))
    .collect::<Result<_, _>>()?;
  let steps = id_texts(&ids)?;
  let mut fresh = false;
  for step in &covering {
    if status_is(step, "green")? && is_str(get(step, "diff_sha")?, cur) {
      fresh = true;
      break;
    }
  }
  let joined = steps.join(", ");
  let line = if covering.is_empty() {
    format!("  {ac}: UNCOVERED (no step references it)")
  } else if fresh {
    format!("  {ac}: covered by {joined}")
  } else {
    format!("  {ac}: UNCOVERED ({joined} not fresh-green)")
  };
  let row = AcCoverage {
    id: ac.to_owned(),
    covered: fresh,
    steps,
  };
  Ok((line, row))
}

/// `pl_ac_coverage_lines LEDGER CUR SPEC`: report-only. A spec-less or AC-less
/// spec prints nothing, and a ledger jq cannot read stops the report with one
/// tagged line; it never fails the caller.
pub fn ac_coverage(ledger: &Ordered, cur: &str, spec: &str) -> CoverageReport {
  let mut report = CoverageReport::default();
  if is_specless(spec) {
    return report;
  }
  let acs = parse_acs(Path::new(spec));
  if acs.is_empty() {
    return report;
  }
  report.stdout.push_str("AC coverage (report-only):\n");
  for ac in acs {
    let Ok((line, row)) = ac_coverage_line(ledger, cur, &ac) else {
      report.stderr.push(format!(
        "plan-ledger: failed to compute AC coverage for {ac}"
      ));
      return report;
    };
    report.stdout.push_str(&line);
    report.stdout.push('\n');
    report.rows.push(row);
  }
  report
}

#[cfg(test)]
#[path = "tests/coverage_test.rs"]
mod tests;
