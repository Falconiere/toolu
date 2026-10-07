//! The AC-coverage report against the text `ledger-model.ts` prints for the
//! same ledger and spec.

use toolu_runtime::json::ordered::Ordered;

use super::{AcCoverage, ac_coverage};
use crate::ledger::jq::parse_json;

fn doc(text: &str) -> Ordered {
  parse_json(text).unwrap()
}

fn spec() -> (tempfile::TempDir, String) {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("spec.md");
  std::fs::write(
    &path,
    "## Acceptance criteria\n- **AC-1:** a\n- **AC-2:** b\n- **AC-3:** c\n",
  )
  .unwrap();
  let path = path.display().to_string();
  (dir, path)
}

#[test]
fn each_ac_is_covered_uncovered_or_unreferenced() {
  let (_dir, spec) = spec();
  let ledger = doc(
    r#"{"steps":[
{"id":"s1","status":"green","diff_sha":"A","ac_refs":["AC-1"]},
{"id":7,"status":"red","diff_sha":"A","ac_refs":["AC-1","AC-2"]},
{"id":null,"status":"green","diff_sha":"B","ac_refs":"AC-2"}]}"#,
  );
  let report = ac_coverage(&ledger, "A", &spec);
  assert_eq!(
    report.stdout,
    "AC coverage (report-only):\n  AC-1: covered by s1, 7\n  AC-2: UNCOVERED (7,  not fresh-green)\n  AC-3: UNCOVERED (no step references it)\n"
  );
  assert_eq!(report.stderr, Vec::<String>::new());
  let row = |id: &str, covered: bool, steps: &[&str]| AcCoverage {
    id: id.to_owned(),
    covered,
    steps: steps.iter().map(|step| (*step).to_owned()).collect(),
  };
  assert_eq!(
    report.rows,
    [
      row("AC-1", true, &["s1", "7"]),
      row("AC-2", false, &["7", ""]),
      row("AC-3", false, &[])
    ]
  );
}

#[test]
fn an_unreadable_ledger_stops_the_report_with_one_line() {
  let (_dir, spec) = spec();
  for ledger in [
    r#"{"steps":[{"id":["x"],"ac_refs":["AC-1"]}]}"#,
    r#"{"steps":[{"id":"a","ac_refs":3}]}"#,
  ] {
    let report = ac_coverage(&doc(ledger), "A", &spec);
    assert_eq!(report.stdout, "AC coverage (report-only):\n");
    assert_eq!(
      report.stderr,
      ["plan-ledger: failed to compute AC coverage for AC-1"]
    );
  }
}

#[test]
fn a_specless_or_ac_less_spec_prints_nothing() {
  let ledger = doc(r#"{"steps":[]}"#);
  assert_eq!(ac_coverage(&ledger, "A", "NONE").stdout, "");
  let dir = tempfile::tempdir().unwrap();
  let plain = dir.path().join("plain.md");
  std::fs::write(&plain, "# no criteria\n").unwrap();
  assert_eq!(
    ac_coverage(&ledger, "A", &plain.display().to_string()).stdout,
    ""
  );
}
