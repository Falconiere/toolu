//! No-mocks (70-no-mocks): mock-framework imports and `mocker` / `monkeypatch`
//! fixture parameters in test files, one `ast-grep scan` of the edited file.
//! Each match shows as `<line>: <its first line>`, cut at a tab as
//! `awk -F'\t'` cut it; without ast-grep the rule is skipped, and a broken scan
//! is itself a violation. Severity is `warning` on purpose: `scan` exits
//! non-zero on an error-level match, which would look like a crashed tool.

use toolu_engine::quality::{AstGrepHit, AstGrepScan, ScanStage, scan_inline};

use toolu_runtime::registry::rule::RuleContext;

use crate::file::{PyFile, head};

/// The two structural rules, as the TypeScript module passed them inline.
pub(crate) const RULES: &str = r"id: mock-import
language: python
severity: warning
message: 'mock import'
rule:
  any:
    - kind: import_statement
      has: {kind: dotted_name, regex: '^(unittest\.mock|mock|pytest_mock)$'}
    - kind: import_from_statement
      has: {field: module_name, kind: dotted_name, regex: '^(unittest\.mock|mock)$'}
    - kind: import_from_statement
      all:
        - has: {field: module_name, kind: dotted_name, regex: '^unittest$'}
        - has: {field: name, kind: dotted_name, regex: '^mock$'}
---
id: mocker-param
language: python
severity: warning
message: 'mocker/monkeypatch fixture parameter'
rule:
  kind: function_definition
  has:
    field: parameters
    has: {kind: identifier, regex: '^(mocker|monkeypatch)$'}";

/// The first five matches of `rule`, as `<line>: <first line>`.
fn shown(hits: &[AstGrepHit], rule: &str) -> String {
  let lines: Vec<String> = hits
    .iter()
    .filter(|hit| hit.rule_id == rule && hit.first)
    .map(|hit| {
      let text = hit.text.split('\t').next().unwrap_or_default();
      format!("{}: {text}", hit.line)
    })
    .collect();
  head(&lines, 5)
}

/// Why the scan cannot be trusted, if it cannot.
fn failure(scan: &AstGrepScan) -> Option<String> {
  match scan {
    AstGrepScan::Ok { empty: true, .. } => Some(
      "ast-grep exited 0 with empty output (expected at least the JSON array \"[]\")".to_owned(),
    ),
    AstGrepScan::Ok { .. } | AstGrepScan::Missing => None,
    AstGrepScan::Failed(failed) if failed.stage == ScanStage::Parse => {
      Some("ast-grep exited 0 but its output did not parse as the documented JSON array".to_owned())
    }
    AstGrepScan::Failed(failed) => {
      let first = if failed.stderr_first.is_empty() {
        String::new()
      } else {
        format!(": {}", failed.stderr_first)
      };
      Some(format!("ast-grep exit {}{first}", failed.exit_code))
    }
  }
}

/// The no-mocks violations of a test file `f` from `scan`.
pub(crate) fn mock_findings(f: &PyFile<'_>, scan: &AstGrepScan) -> Vec<String> {
  let path = &f.file.path;
  let hits = match scan {
    AstGrepScan::Ok { hits, .. } => hits.as_slice(),
    AstGrepScan::Missing => return Vec::new(),
    AstGrepScan::Failed(_) => &[],
  };
  let mut errors = Vec::new();
  let imports = shown(hits, "mock-import");
  if !imports.is_empty() {
    errors.push(format!(
      "no-mocks: mock import in {path} — write against real data/fixtures instead\n{imports}"
    ));
  }
  let params = shown(hits, "mocker-param");
  if !params.is_empty() {
    errors.push(format!(
      "no-mocks: mocker/monkeypatch fixture parameter in {path} — write against real \
       data/fixtures instead\n{params}"
    ));
  }
  if let Some(broken) = failure(scan) {
    errors.push(format!(
      "ast-grep failed while scanning {path} — {broken}; no-mocks rules could not be verified. \
       Fix the tool/file and re-edit"
    ));
  }
  errors
}

/// Only for `test_*.py` / `*_test.py`, unless `lang.python.noMocks` is false:
/// then one scan of the file.
pub(crate) fn no_mocks(f: &PyFile<'_>, ctx: &RuleContext<'_>, is_test: bool) -> Vec<String> {
  if !is_test || !f.limits.no_mocks {
    return Vec::new();
  }
  let scan = scan_inline(std::slice::from_ref(f.file), RULES, ctx);
  mock_findings(f, &scan)
}

#[cfg(test)]
#[path = "tests/no_mocks_test.rs"]
mod tests;
