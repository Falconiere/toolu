use std::path::PathBuf;

use toolu_engine::ledger::coverage::{AcCoverage, CoverageReport};
use toolu_engine::ledger::io::CommandResult;
use toolu_protocol::exit::Exit;
use toolu_runtime::json::ordered::Ordered;

use super::{exit_of, located, ran, reported, self_tested, statused, verdict};

fn result(exit: u8, stdout: &str, stderr: &str, ledger: Option<Ordered>) -> CommandResult {
  CommandResult {
    exit,
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
    ledger,
  }
}

fn ledger() -> Ordered {
  Ordered::parse(r#"{"version":1,"steps":[]}"#).unwrap()
}

#[test]
fn exits_and_text_keep_the_typescript_bytes() {
  assert_eq!(
    [exit_of(0), exit_of(1), exit_of(2)],
    [Exit::Success, Exit::Failure, Exit::Blocked]
  );
  let shown = reported(&result(1, "line\n", "a\nb\n", None), None);
  assert_eq!(
    (shown.stdout.as_deref(), shown.stderr.as_deref()),
    (Some("line"), Some("a\nb"))
  );
  let silent = reported(&result(0, "", "", None), None);
  assert_eq!((silent.stdout, silent.stderr), (None, None));
  let blank = reported(&result(0, "\n", "", None), None);
  assert_eq!(blank.stdout.as_deref(), Some(""));
}

#[test]
fn run_and_status_documents_carry_the_ledger() {
  let line = "plan-ledger feat_x: 1/1 fresh-green, next=none\n";
  let document = ran(&result(0, line, "", Some(ledger())), true);
  assert_eq!(
    document.stdout.as_deref(),
    Some(
      r#"{"summary":"plan-ledger feat_x: 1/1 fresh-green, next=none","ledger":{"version":1,"steps":[]}}"#
    )
  );
  assert_eq!(
    ran(&result(0, line, "", Some(ledger())), false)
      .stdout
      .as_deref(),
    Some(line.trim_end())
  );
  let no_line = ran(&result(1, "", "", Some(ledger())), true);
  assert_eq!(
    no_line.stdout.as_deref(),
    Some(r#"{"summary":null,"ledger":{"version":1,"steps":[]}}"#)
  );
  assert_eq!(
    ran(&result(2, "", "plan-ledger: x\n", None), true).stdout,
    None
  );
  let report = CoverageReport {
    stdout: String::new(),
    stderr: Vec::new(),
    rows: vec![AcCoverage {
      id: "AC-1".to_owned(),
      covered: false,
      steps: vec!["s1".to_owned()],
    }],
  };
  let status = statused(
    (
      result(1, "AC coverage (report-only):\n", "", Some(ledger())),
      report,
    ),
    true,
  );
  assert_eq!(
    status.stdout.as_deref(),
    Some(
      r#"{"summary":null,"ledger":{"version":1,"steps":[]},"ac_coverage":[{"id":"AC-1","covered":false,"steps":["s1"]}]}"#
    )
  );
}

#[test]
fn paths_self_test_and_verdict_documents() {
  assert_eq!(
    located(Ok(PathBuf::from("/r/x.json")), "path", false)
      .stdout
      .as_deref(),
    Some("/r/x.json")
  );
  assert_eq!(
    located(Ok(PathBuf::from("/r")), "root", true)
      .stdout
      .as_deref(),
    Some(r#"{"root":"/r"}"#)
  );
  let missing = located(
    Err(result(2, "", "plan-ledger: not in a git repo\n", None)),
    "root",
    true,
  );
  assert_eq!((missing.exit, missing.stdout), (Exit::Blocked, None));
  assert_eq!(
    self_tested(&result(1, "", "boom\n", None), true).stdout,
    None
  );
  let report = Ordered::parse(r#"{"overall":"ready"}"#).unwrap();
  let json = verdict(
    &result(
      0,
      "{\n  \"overall\": \"ready\"\n}\n",
      "",
      Some(report.clone()),
    ),
    true,
  );
  assert_eq!(json.stdout.as_deref(), Some(r#"{"overall":"ready"}"#));
  let table = verdict(&result(1, "verdict: x\n", "", Some(report)), false);
  assert_eq!(
    (table.exit, table.stdout.as_deref()),
    (Exit::Failure, Some("verdict: x"))
  );
}
