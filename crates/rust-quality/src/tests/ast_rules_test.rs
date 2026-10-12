use std::path::Path;

use toolu_engine::quality::{AstGrepHit, AstGrepScan, EditedFile, ScanFailure, ScanStage};
use toolu_runtime::env::Env;

use super::{error_handling, error_rules, mock_rules, no_mocks};
use crate::ast_yaml::{ERROR_RULES, MOCK_SRC_RULES, MOCK_TEST_RULES};
use crate::file::{Limits, RsFile};

/// What `rules` makes of `path` under `limits`.
fn on<T>(path: &str, limits: &Limits, rules: impl Fn(&RsFile<'_>) -> T) -> T {
  let file = EditedFile {
    path: path.into(),
    absolute: Path::new(path).into(),
    removed: false,
  };
  let env = Env::default();
  rules(&RsFile {
    file: &file,
    scope: path,
    lines: Vec::new(),
    limits,
    env: &env,
    cwd: Path::new("/"),
  })
}

fn found(rule: &str, row: i64) -> AstGrepHit {
  let excerpt = format!("/r/src/a.rs:{row}:  x.{rule}()\t\t");
  AstGrepHit {
    rule_id: rule.into(),
    file: "/r/src/a.rs".into(),
    line: row,
    text: String::new(),
    excerpt,
    first: true,
  }
}

fn broken(stage: ScanStage, stderr_first: &str) -> AstGrepScan {
  AstGrepScan::Failed(ScanFailure {
    stage,
    exit_code: 3,
    stderr_first: stderr_first.into(),
  })
}

#[test]
fn each_file_gets_the_rule_yaml_the_typescript_composed() {
  let on_mocks = Limits::default();
  let off = Limits {
    no_mocks: false,
    ..Limits::default()
  };
  assert_eq!(
    on("/r/src/a.rs", &on_mocks, |f| error_rules(f, false)),
    Some(ERROR_RULES)
  );
  assert_eq!(on("/r/src/a.rs", &on_mocks, |f| error_rules(f, true)), None);
  assert_eq!(
    on("/r/build.rs", &on_mocks, |f| error_rules(f, false)),
    None
  );
  assert_eq!(
    on("/r/src/a.rs", &on_mocks, mock_rules).as_deref(),
    Some(MOCK_SRC_RULES)
  );
  assert_eq!(
    on("/r/tests/a.rs", &on_mocks, mock_rules).as_deref(),
    Some(MOCK_TEST_RULES)
  );
  let both = format!("{MOCK_SRC_RULES}\n---\n{MOCK_TEST_RULES}");
  assert_eq!(
    on("/r/src/tests/a_test.rs", &on_mocks, mock_rules),
    Some(both)
  );
  assert_eq!(on("/r/src/a.rs", &off, mock_rules), None);
  assert_eq!(on("/r/examples/a.rs", &on_mocks, mock_rules), None);
}

#[test]
fn error_hits_are_grouped_and_capped_per_rule() {
  let mut hits: Vec<AstGrepHit> = (1..=6).map(|row| found("unwrap", row)).collect();
  hits.extend([found("todo", 7), found("panic", 8), found("expect", 9)]);
  let scan = AstGrepScan::Ok { hits, empty: false };
  let out = on("/r/src/a.rs", &Limits::default(), |f| {
    error_handling(f, Some(&scan))
  });
  assert_eq!(out.len(), 3);
  assert!(out[0].starts_with(".unwrap() in /r/src/a.rs — use ? or match on Result/Option\n"));
  assert_eq!(
    out[0].lines().count(),
    6,
    "five unwraps at most: {}",
    out[0]
  );
  assert!(
    out[0].ends_with("/r/src/a.rs:5:  x.unwrap()"),
    "trailing tabs are dropped"
  );
  assert!(out[1].starts_with(".expect() in /r/src/a.rs"));
  assert_eq!(
    out[2],
    "panic!/todo!/unimplemented!/unreachable! in /r/src/a.rs — return a Result instead\n/r/src/a.rs:8:  x.panic()\n/r/src/a.rs:7:  x.todo()"
  );
  let none: [String; 0] = [];
  assert_eq!(
    on("/r/src/a.rs", &Limits::default(), |f| error_handling(
      f, None
    )),
    none
  );
  let missing = Some(&AstGrepScan::Missing);
  assert_eq!(
    on("/r/src/a.rs", &Limits::default(), |f| error_handling(
      f, missing
    )),
    none
  );
}

#[test]
fn a_broken_scan_fails_each_group_and_blank_output_fails_no_mocks_only() {
  let limits = Limits::default();
  let crashed = broken(ScanStage::AstGrep, "boom");
  let errors = on("/r/src/a.rs", &limits, |f| {
    error_handling(f, Some(&crashed))
  });
  assert_eq!(
    errors,
    [
      "ast-grep failed while scanning /r/src/a.rs — ast-grep exit 3: boom; error-handling rules could not be verified. Fix the tool/file and re-edit"
    ]
  );
  let mocks = on("/r/src/a.rs", &limits, |f| {
    no_mocks(f, Some(&broken(ScanStage::AstGrep, "")))
  });
  assert_eq!(
    mocks,
    [
      "ast-grep failed while scanning /r/src/a.rs — ast-grep exit 3; no-mocks rules could not be verified. Fix the tool/file and re-edit"
    ]
  );
  let unparsed = broken(ScanStage::Parse, "");
  let out = on("/r/src/a.rs", &limits, |f| no_mocks(f, Some(&unparsed)));
  assert!(out[0].contains(
    "— ast-grep exited 0 but its output did not parse as the documented JSON array; no-mocks"
  ));
  let blank = AstGrepScan::Ok {
    hits: Vec::new(),
    empty: true,
  };
  let none: [String; 0] = [];
  assert_eq!(
    on("/r/src/a.rs", &limits, |f| error_handling(f, Some(&blank))),
    none
  );
  let out = on("/r/src/a.rs", &limits, |f| no_mocks(f, Some(&blank)));
  assert!(out[0].contains(r#"— ast-grep exited 0 with empty output (expected at least the JSON array "[]"); no-mocks rules"#));
}

#[test]
fn mock_hits_are_grouped_by_kind() {
  let hits = vec![
    found("mock-macro", 2),
    found("automock-attr", 1),
    found("faux-import", 3),
  ];
  let scan = AstGrepScan::Ok { hits, empty: false };
  let out = on("/r/src/a.rs", &Limits::default(), |f| {
    no_mocks(f, Some(&scan))
  });
  let tail = "— write against real data/fixtures instead";
  assert_eq!(
    out,
    [
      format!(
        "no-mocks: #[automock] mock definition in /r/src/a.rs {tail}\n/r/src/a.rs:1:  x.automock-attr()"
      ),
      format!(
        "no-mocks: mock! {{ ... }} mock definition in /r/src/a.rs {tail}\n/r/src/a.rs:2:  x.mock-macro()"
      ),
      format!(
        "no-mocks: mockall/faux import in /r/src/a.rs {tail}\n/r/src/a.rs:3:  x.faux-import()"
      ),
    ]
  );
}
