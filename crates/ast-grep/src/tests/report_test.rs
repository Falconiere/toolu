use super::{parse, render};

#[test]
fn savings_report_matches_every_recorded_fixture_ledger() {
  let fixture = std::fs::read_to_string(
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ast-grep/report.json"),
  )
  .expect("report fixture");
  let cases: serde_json::Value = serde_json::from_str(&fixture).expect("report fixture JSON");
  let golden: serde_json::Value =
    serde_json::from_str(include_str!("../../../../fixtures/ast-grep/golden.json"))
      .expect("report golden");
  for case in cases["cases"].as_array().expect("report cases") {
    let Some(ledger) = case["ledger"].as_str() else {
      continue;
    };
    let name = case["name"].as_str().expect("case name");
    let expected = golden["report"][name]["stdout"]
      .as_str()
      .expect("golden stdout");
    assert_eq!(
      render(&parse(ledger).expect("valid ledger")),
      expected.trim_end(),
      "{name}"
    );
  }
}

#[test]
fn savings_report_names_first_invalid_line() {
  assert_eq!(
    parse("{\"kind\":\"grep\",\"returned\":8,\"full\":0}\nwrong\n").err(),
    Some(2)
  );
}
