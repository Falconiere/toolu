//! `cargo xtask check-coverage <llvm-cov.json>`: line coverage per crate against
//! its floor (rule 9): `coverage.default`, a `coverage.strict` row, or a higher
//! `coverage-floor.json` row, whichever is highest.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::data::{self, CoverageFloor, Rules};
use crate::options::Options;
use crate::workspace::{Workspace, relative};
use crate::{Verdict, output};

/// `cargo llvm-cov report --json --summary-only`, the fields read here.
#[derive(Debug, Deserialize)]
pub(crate) struct Export {
  pub(crate) data: Vec<ExportData>,
}

/// One export section.
#[derive(Debug, Deserialize)]
pub(crate) struct ExportData {
  pub(crate) files: Vec<FileSummary>,
}

/// One file's summary.
#[derive(Debug, Deserialize)]
pub(crate) struct FileSummary {
  pub(crate) filename: String,
  pub(crate) summary: Summary,
}

/// The per-file totals.
#[derive(Debug, Deserialize)]
pub(crate) struct Summary {
  pub(crate) lines: Count,
}

/// Covered out of total.
#[derive(Debug, Deserialize)]
pub(crate) struct Count {
  pub(crate) count: u64,
  pub(crate) covered: u64,
}

/// Judge the export named by the first positional argument.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let file = options
    .files
    .first()
    .ok_or("check-coverage needs the llvm-cov JSON file")?;
  let text = std::fs::read_to_string(file)
    .map_err(|err| format!("cannot read {}: {err}", file.display()))?;
  let export: Export =
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", file.display()))?;
  check(options, &export)
}

/// Judge `export` against the floors of the workspace at `options.root`.
pub(crate) fn check(options: &Options, export: &Export) -> Result<Verdict, String> {
  let workspace = Workspace::load(&options.root)?;
  let rules = data::rules(&options.root)?;
  let floors: CoverageFloor = data::load(&options.root, "coverage-floor.json")?;
  let found = judge(&workspace, export, &rules, &floors);
  Ok(output::findings("check-coverage", &found))
}

/// The floor that applies to `krate`.
pub(crate) fn floor(rules: &Rules, floors: &CoverageFloor, krate: &str) -> f64 {
  let strict = rules
    .coverage
    .strict
    .get(krate)
    .copied()
    .unwrap_or(rules.coverage.default);
  let row = floors.floors.get(krate).copied().unwrap_or(0.0);
  rules.coverage.default.max(strict).max(row)
}

/// One line per crate under its floor.
pub(crate) fn judge(
  workspace: &Workspace,
  export: &Export,
  rules: &Rules,
  floors: &CoverageFloor,
) -> Vec<String> {
  let mut totals: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
  for file in export.data.iter().flat_map(|data| &data.files) {
    let rel = relative(&workspace.root, Path::new(&file.filename));
    let Some(member) = workspace.member_of(&rel) else {
      continue;
    };
    let Ok(inside) = rel.strip_prefix(member.dir.join("src")) else {
      continue;
    };
    if inside.components().any(|part| part.as_os_str() == "tests") {
      continue;
    }
    let total = totals.entry(member.name.as_str()).or_insert((0, 0));
    total.0 += file.summary.lines.covered;
    total.1 += file.summary.lines.count;
  }
  let mut found = Vec::new();
  for (krate, (covered, count)) in totals {
    if count == 0 {
      continue;
    }
    let percent = ratio(covered, count);
    let wanted = floor(rules, floors, krate);
    if percent < wanted {
      found.push(format!(
        "coverage {krate}: {percent:.1}% lines, floor {wanted}% — add tests"
      ));
    }
  }
  found
}

/// `covered / count` in percent; both are line counts far below 2^52.
fn ratio(covered: u64, count: u64) -> f64 {
  let covered = u32::try_from(covered).map_or(f64::MAX, f64::from);
  let count = u32::try_from(count).map_or(f64::MAX, f64::from);
  covered * 100.0 / count
}

#[cfg(test)]
#[path = "tests/coverage_test.rs"]
mod tests;
