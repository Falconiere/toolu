use super::{MAX_LEDGER_BYTES, parse, read_ledger, render};

#[test]
fn ledger_reader_accepts_normal_jsonl_and_rejects_oversized_file() {
  let file = tempfile::NamedTempFile::new().expect("temporary ledger");
  let line = b"{\"kind\":\"grep\",\"returned\":8,\"full\":0}\n";
  std::fs::write(file.path(), line).expect("write ledger");
  assert_eq!(
    read_ledger(file.path()).expect("normal ledger"),
    String::from_utf8_lossy(line)
  );

  file
    .as_file()
    .set_len(MAX_LEDGER_BYTES + 1)
    .expect("extend ledger");
  let error = read_ledger(file.path()).expect_err("oversized ledger rejected");
  assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
  assert!(error.to_string().contains("ledger exceeds 16777216 bytes"));
}

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
