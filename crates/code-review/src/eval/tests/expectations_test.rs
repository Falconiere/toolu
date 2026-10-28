use super::{CaseSet, Class, Kind};

const FILE: &str = r#"{
  "version": 1,
  "cases": [
    {
      "id": "slice-underflow",
      "class": "supported",
      "base": "HEAD~1",
      "memory": null,
      "expectations": [
        {"id": "defect-underflow", "path": "src/lib.rs", "claim": "slice underflows", "kind": "defect"},
        {"id": "fp-rewrite", "path": "src/lib.rs", "claim": "prefer rewrite", "kind": "false_positive"}
      ],
      "gate": {"blocking_severities_drive_count": true, "advisory_never_blocks": true}
    },
    {"id": "drift", "class": "memory_drift", "expectations": [
      {"id": "kept-open", "path": "src/lib.rs", "claim": "stale", "kind": "defect"}
    ]}
  ]
}"#;

#[test]
fn the_spec_example_parses_into_cases_classes_and_kinds() {
  let set = CaseSet::parse(FILE).unwrap();
  assert_eq!(set.cases.len(), 2);
  let case = set.case("slice-underflow").unwrap();
  assert_eq!(case.class, Class::Supported);
  assert_eq!(case.base.as_deref(), Some("HEAD~1"));
  let kinds: Vec<Kind> = case.expectations.iter().map(|row| row.kind).collect();
  assert_eq!(kinds, [Kind::Defect, Kind::FalsePositive]);
  let gate = case.gate.unwrap();
  assert!(gate.blocking_severities_drive_count && gate.advisory_never_blocks);
  assert!(!gate.unreviewed_never_writes_pass);
  assert_eq!(set.case("drift").unwrap().class, Class::MemoryDrift);
  assert!(set.case("missing").is_none());
}

#[test]
fn a_set_round_trips_through_json() {
  let set = CaseSet::parse(FILE).unwrap();
  let again = CaseSet::parse(&serde_json::to_string(&set).unwrap()).unwrap();
  assert_eq!(set, again);
}

#[test]
fn an_unknown_version_is_refused() {
  let error = CaseSet::parse(r#"{"version": 2, "cases": []}"#).unwrap_err();
  assert!(error.contains("version 2"), "{error}");
}

#[test]
fn malformed_json_and_unknown_kinds_are_errors() {
  assert!(CaseSet::parse("not json").is_err());
  let bad = FILE.replace("\"defect\"", "\"typo\"");
  assert!(CaseSet::parse(&bad).is_err());
}

fn one_case(id: &str, rows: &str) -> String {
  format!(
    r#"{{"version": 1, "cases": [{{"id": "{id}", "class": "supported", "expectations": [{rows}]}}]}}"#
  )
}

const ROW: &str = r#"{"id": "d", "path": "a.rs", "claim": "x", "kind": "defect"}"#;

#[test]
fn an_empty_set_is_refused() {
  let error = CaseSet::parse(r#"{"version": 1, "cases": []}"#).unwrap_err();
  assert!(error.contains("no cases"), "{error}");
}

#[test]
fn a_case_without_expectations_is_refused() {
  let error = CaseSet::parse(&one_case("c", "")).unwrap_err();
  assert!(error.contains("case c has no expectations"), "{error}");
}

#[test]
fn duplicate_case_ids_and_duplicate_expectation_ids_are_refused() {
  let twice = format!(
    r#"{{"version": 1, "cases": [
      {{"id": "c", "class": "supported", "expectations": [{ROW}]}},
      {{"id": "c", "class": "supported", "expectations": [{ROW}]}}
    ]}}"#
  );
  let error = CaseSet::parse(&twice).unwrap_err();
  assert!(error.contains("duplicate case id c"), "{error}");
  let error = CaseSet::parse(&one_case("c", &format!("{ROW},{ROW}"))).unwrap_err();
  assert!(error.contains("repeats expectation id d"), "{error}");
}

#[test]
fn a_case_id_must_be_a_plain_name() {
  assert!(CaseSet::parse(&one_case("ok_Case-1", ROW)).is_ok());
  for bad in ["", "../escape", "a/b", "has space", "dot.ted"] {
    let error = CaseSet::parse(&one_case(bad, ROW)).unwrap_err();
    assert!(
      error.contains("must match [A-Za-z0-9_-]+"),
      "{bad}: {error}"
    );
  }
}
