//! The shared edit-records golden (AC-8): `fixtures/state/edit-records.json`
//! was captured once from the TypeScript normalizer; every payload yields the
//! same kind, records and printed text from Rust.
//! (`packages/toolu-core/src/state/__tests__/edit-records.test.ts` holds
//! TypeScript to the same golden.)

use std::path::Path;

use serde_json::{Value, json};
use toolu_state::edit_records::{
  EditRecord, EditRecords, format_edit_records, normalize_edit_records,
};

type Res<T> = Result<T, String>;

fn cases() -> Res<Vec<Value>> {
  let file =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/state/edit-records.json");
  let text = std::fs::read_to_string(&file).map_err(|err| err.to_string())?;
  let doc: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
  let cases = doc
    .get("cases")
    .and_then(Value::as_array)
    .ok_or("no cases")?;
  Ok(cases.clone())
}

fn record_json(record: &EditRecord) -> Value {
  let mut value = json!({ "path": record.path, "operation": record.operation.name() });
  if let (Some(to), Some(object)) = (&record.moved_to, value.as_object_mut()) {
    object.insert("moved_to".to_owned(), json!(to));
  }
  if let (Some(from), Some(object)) = (&record.from, value.as_object_mut()) {
    object.insert("from".to_owned(), json!(from));
  }
  value
}

/// What Rust makes of `case`, in the golden's `expect` shape.
fn ours(case: &Value) -> Res<Value> {
  let tool = case.get("tool").and_then(Value::as_str).ok_or("no tool")?;
  let payload = case.get("payload").ok_or("no payload")?;
  Ok(match normalize_edit_records(payload, tool) {
    EditRecords::Records(records) => json!({
      "kind": "records",
      "records": records.iter().map(record_json).collect::<Vec<_>>(),
      "text": format_edit_records(&records),
    }),
    EditRecords::NotEdit => json!({ "kind": "not-edit" }),
    EditRecords::Malformed => json!({ "kind": "malformed" }),
  })
}

#[test]
fn every_golden_payload_normalizes_as_typescript_does() {
  let cases = cases().unwrap();
  assert_eq!(cases.len(), 42);
  for case in &cases {
    assert_eq!(
      Some(&ours(case).unwrap()),
      case.get("expect"),
      "{}",
      case["name"]
    );
  }
}
