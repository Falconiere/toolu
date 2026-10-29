//! The structural rules: panic-on-error in production code (60) and mock
//! frameworks (70), read from the shared `ast-grep scan`. Without ast-grep
//! they are skipped; a broken scan is itself a violation. Hits are filtered per
//! rule, which keeps ast-grep's order stable.

use toolu_engine::quality::{AstGrepHit, AstGrepScan, ScanStage};

use crate::ast_yaml::{ERROR_RULES, MOCK_SRC_RULES, MOCK_TEST_RULES};
use crate::file::{RsFile, test_file_name};

/// The error-handling rules `f` is scanned with: non-test files under src/.
pub(crate) fn error_rules(f: &RsFile<'_>, is_test: bool) -> Option<&'static str> {
  (f.path_has("/src/") && !is_test).then_some(ERROR_RULES)
}

/// The no-mocks rules `f` is scanned with: definitions in src/, imports in
/// test files, unless `lang.rust.noMocks` is false.
pub(crate) fn mock_rules(f: &RsFile<'_>) -> Option<String> {
  if !f.limits.no_mocks {
    return None;
  }
  let in_tests = f.path_has("/tests/") || test_file_name(f.path());
  let parts: Vec<&str> = [
    f.path_has("/src/").then_some(MOCK_SRC_RULES),
    in_tests.then_some(MOCK_TEST_RULES),
  ]
  .into_iter()
  .flatten()
  .collect();
  (!parts.is_empty()).then(|| parts.join("\n---\n"))
}

fn hits(scan: &AstGrepScan) -> &[AstGrepHit] {
  match scan {
    AstGrepScan::Ok { hits, .. } => hits,
    AstGrepScan::Missing | AstGrepScan::Failed(_) => &[],
  }
}

/// `header` with its excerpts, each rule's capped; `read` drops trailing tabs.
fn group(f: &RsFile<'_>, scan: &AstGrepScan, header: &str, caps: &[(&str, usize)]) -> Vec<String> {
  let mut text = header.replacen("$F", f.path(), 1);
  let before = text.len();
  for (rule, cap) in caps {
    for found in hits(scan)
      .iter()
      .filter(|found| found.rule_id == *rule)
      .take(*cap)
    {
      text.push('\n');
      text.push_str(found.excerpt.trim_end_matches('\t'));
    }
  }
  if text.len() == before {
    Vec::new()
  } else {
    vec![text]
  }
}

fn failure_detail(scan: &AstGrepScan) -> Option<String> {
  let AstGrepScan::Failed(failure) = scan else {
    return None;
  };
  if failure.stage == ScanStage::Parse {
    return Some(
      "ast-grep exited 0 but its output did not parse as the documented JSON array".to_owned(),
    );
  }
  let first = if failure.stderr_first.is_empty() {
    String::new()
  } else {
    format!(": {}", failure.stderr_first)
  };
  Some(format!("ast-grep exit {}{first}", failure.exit_code))
}

fn failure(f: &RsFile<'_>, detail: Option<String>, rules: &str) -> Vec<String> {
  detail
    .map(|detail| {
      format!(
        "ast-grep failed while scanning {} — {detail}; {rules} rules could not be verified. Fix the tool/file and re-edit",
        f.path()
      )
    })
    .into_iter()
    .collect()
}

/// 60-error-handling over the scan of [`error_rules`]; `None` when it does not apply.
pub(crate) fn error_handling(f: &RsFile<'_>, scan: Option<&AstGrepScan>) -> Vec<String> {
  let Some(scan) = scan.filter(|scan| **scan != AstGrepScan::Missing) else {
    return Vec::new();
  };
  [
    group(
      f,
      scan,
      ".unwrap() in $F — use ? or match on Result/Option",
      &[("unwrap", 5)],
    ),
    group(
      f,
      scan,
      ".expect() in $F — use ? or match on Result/Option",
      &[("expect", 5)],
    ),
    group(
      f,
      scan,
      "panic!/todo!/unimplemented!/unreachable! in $F — return a Result instead",
      &[
        ("panic", 3),
        ("todo", 3),
        ("unimplemented", 3),
        ("unreachable", 3),
      ],
    ),
    failure(f, failure_detail(scan), "error-handling"),
  ]
  .concat()
}

/// 70-no-mocks over the scan of [`mock_rules`]; `None` when it does not apply.
pub(crate) fn no_mocks(f: &RsFile<'_>, scan: Option<&AstGrepScan>) -> Vec<String> {
  let Some(scan) = scan.filter(|scan| **scan != AstGrepScan::Missing) else {
    return Vec::new();
  };
  let detail = if matches!(scan, AstGrepScan::Ok { empty: true, .. }) {
    Some(
      r#"ast-grep exited 0 with empty output (expected at least the JSON array "[]")"#.to_owned(),
    )
  } else {
    failure_detail(scan)
  };
  let tail = "— write against real data/fixtures instead";
  let in_src = f.path_has("/src/");
  let in_tests = f.path_has("/tests/") || test_file_name(f.path());
  [
    if in_src {
      group(
        f,
        scan,
        &format!("no-mocks: #[automock] mock definition in $F {tail}"),
        &[("automock-attr", 5), ("automock-cfg-attr", 5)],
      )
    } else {
      Vec::new()
    },
    if in_src {
      group(
        f,
        scan,
        &format!("no-mocks: mock! {{ ... }} mock definition in $F {tail}"),
        &[("mock-macro", 5)],
      )
    } else {
      Vec::new()
    },
    if in_tests {
      group(
        f,
        scan,
        &format!("no-mocks: mockall/faux import in $F {tail}"),
        &[("mockall-import", 5), ("faux-import", 5)],
      )
    } else {
      Vec::new()
    },
    failure(f, detail, "no-mocks"),
  ]
  .concat()
}

#[cfg(test)]
#[path = "tests/ast_rules_test.rs"]
mod tests;
