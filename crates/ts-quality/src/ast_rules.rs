//! The structural rules: error handling (78) and mock calls in tests (85),
//! read from the event's one `ast-grep scan`. Without ast-grep they are
//! skipped; a broken scan is itself a violation.

use toolu_engine::quality::{AstGrepHit, AstGrepScan, ScanFailure, ScanStage};
use toolu_protocol::host::Host;

use crate::ast_yaml::{ERROR_RULES, MOCK_RULES};
use crate::file::{Pattern, TsFile, is_test_path};

/// The rules for the event's scan. ast-grep picks rules by file language, so
/// `OpenCode` adds the `tsx` copies its `.tsx` files were always checked with;
/// elsewhere a `.tsx` file matches no rule, as before.
pub(crate) fn inline_rules(host: Host) -> String {
  let ts = format!("{ERROR_RULES}\n---\n{MOCK_RULES}");
  if host != Host::Opencode {
    return ts;
  }
  let tsx = ts.replace("language: ts\n", "language: tsx\n");
  format!("{ts}\n---\n{tsx}")
}

const MOCK_IDS: [&str; 5] = ["jest-mock", "vi-mock", "jest-fn", "vi-fn", "sinon-method"];

fn hits(scan: &AstGrepScan) -> &[AstGrepHit] {
  match scan {
    AstGrepScan::Ok { hits, .. } => hits,
    AstGrepScan::Missing | AstGrepScan::Failed(_) => &[],
  }
}

/// One rule's excerpts, capped; `read`'s field split drops trailing tabs.
fn excerpts(all: &[AstGrepHit], rule: &str, limit: usize) -> Vec<String> {
  all
    .iter()
    .filter(|hit| hit.rule_id == rule)
    .take(limit)
    .map(|hit| hit.excerpt.trim_end_matches('\t').to_owned())
    .collect()
}

fn group(path: &str, all: &[AstGrepHit], header: &str, rules: &[(&str, usize)]) -> Option<String> {
  let lines: Vec<String> = rules
    .iter()
    .flat_map(|(rule, limit)| excerpts(all, rule, *limit))
    .collect();
  (!lines.is_empty()).then(|| format!("{}\n{}", header.replace("$F", path), lines.join("\n")))
}

fn first_line(failure: &ScanFailure) -> String {
  if failure.stderr_first.is_empty() {
    String::new()
  } else {
    format!(": {}", failure.stderr_first)
  }
}

fn scan_failure(scan: &AstGrepScan) -> Option<String> {
  let AstGrepScan::Failed(failure) = scan else {
    return None;
  };
  Some(match failure.stage {
    ScanStage::Parse => {
      "ast-grep exited 0 but its output did not parse as the documented JSON array".to_owned()
    }
    ScanStage::AstGrep => format!("ast-grep exit {}{}", failure.exit_code, first_line(failure)),
  })
}

const GROUPS: [(&str, &[(&str, usize)]); 5] = [
  (
    "Empty catch block in $F — handle the error or rethrow; do not swallow",
    &[("empty-catch", 3), ("empty-catch-noarg", 3)],
  ),
  (
    "Silent promise rejection in $F — log or rethrow the error",
    &[
      ("empty-catch-handler", 3),
      ("null-catch-handler", 3),
      ("undef-catch-handler", 3),
    ],
  ),
  (
    "Catch swallows the error by returning a nullish value in $F — handle, log, or rethrow it",
    &[
      ("swallow-null-arg", 2),
      ("swallow-undef-arg", 2),
      ("swallow-null", 2),
      ("swallow-undef", 2),
      ("swallow-bare", 2),
    ],
  ),
  (
    "throw new Error() with no message in $F — include a descriptive message",
    &[("throw-empty-error", 3)],
  ),
  (
    "throw of string literal in $F — throw an Error (or subclass) instead",
    &[("throw-string", 3), ("throw-template", 3)],
  ),
];

/// 78-error-ast.
pub(crate) fn error_handling(f: &TsFile<'_>, scan: &AstGrepScan) -> Vec<String> {
  if *scan == AstGrepScan::Missing {
    return Vec::new();
  }
  let all = hits(scan);
  let mut errors: Vec<String> = GROUPS
    .iter()
    .filter_map(|(header, rules)| group(f.path(), all, header, rules))
    .collect();
  if let Some(failure) = scan_failure(scan) {
    errors.push(format!(
      "ast-grep failed while scanning {} — {failure}; error-handling rules could not be verified. Fix the tool/file and re-edit",
      f.path()
    ));
  }
  errors
}

fn mock_scan_failure(path: &str, scan: &AstGrepScan) -> Option<String> {
  let at = format!("ast-grep failed while scanning {path} for mocks —");
  let tail = "no-mocks rule could not be verified. Fix the tool/file and re-edit";
  match scan {
    AstGrepScan::Ok { empty: true, .. } => Some(format!(
      "{at} exited 0 with empty output (expected at least the JSON array \"[]\"); {tail}"
    )),
    AstGrepScan::Failed(failure) if failure.stage == ScanStage::Parse => Some(format!(
      "{at} its output did not parse as the documented JSON array; {tail}"
    )),
    AstGrepScan::Failed(failure) => Some(format!(
      "{at} exit {}{}; {tail}",
      failure.exit_code,
      first_line(failure)
    )),
    AstGrepScan::Ok { .. } | AstGrepScan::Missing => None,
  }
}

static TS_MOCKITO: Pattern = Pattern::new(r#"from[[:space:]]+["']ts-mockito["']"#);

/// 85-no-mocks, for test files outside e2e unless `lang.ts.noMocks` is false.
/// Hits are listed in source order.
pub(crate) fn mock_doubles(f: &TsFile<'_>, scan: &AstGrepScan) -> Vec<String> {
  let path = f.path();
  let in_tests = is_test_path(path) || path.contains("/__tests__/");
  if path.contains("/e2e/") || !in_tests || !f.limits.no_mocks {
    return Vec::new();
  }
  let mut errors: Vec<String> = mock_scan_failure(path, scan).into_iter().collect();
  let mut found: Vec<&AstGrepHit> = hits(scan)
    .iter()
    .filter(|hit| MOCK_IDS.contains(&hit.rule_id.as_str()))
    .collect();
  found.sort_by_key(|hit| hit.line);
  if !found.is_empty() {
    let excerpt = found
      .iter()
      .take(5)
      .map(|hit| hit.excerpt.as_str())
      .collect::<Vec<_>>()
      .join("\n");
    errors.push(format!(
      "Mocked test double in {path} — tests must exercise real data/services, not mocks/stubs (jest.mock/vi.mock/jest.fn/vi.fn/sinon)\n{excerpt}"
    ));
  }
  if f.lines.iter().any(|line| TS_MOCKITO.is_match(line)) {
    errors.push(format!(
      "Import from ts-mockito in {path} — tests must exercise real data/services, not mocks/stubs"
    ));
  }
  errors
}

#[cfg(test)]
#[path = "tests/ast_rules_test.rs"]
mod tests;
