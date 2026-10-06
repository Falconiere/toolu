use serde_json::json;

use super::{
  EditOperation, EditRecord, EditRecords, format_edit_records, is_edit_tool, normalize_edit_records,
};

fn one(path: &str, operation: EditOperation) -> EditRecords {
  EditRecords::Records(vec![EditRecord {
    path: path.into(),
    operation,
    moved_to: None,
    from: None,
  }])
}

#[test]
fn only_the_four_edit_tools_are_edits() {
  for tool in ["Edit", "Write", "MultiEdit", "apply_patch"] {
    assert!(is_edit_tool(tool));
  }
  assert!(!is_edit_tool("Read"));
  assert_eq!(
    normalize_edit_records(&json!({}), "Bash"),
    EditRecords::NotEdit
  );
}

#[test]
fn the_path_comes_from_the_first_truthy_field() {
  let payload = json!({ "tool_input": { "file_path": false, "path": null, "target_file": "/t" } });
  assert_eq!(
    normalize_edit_records(&payload, "Edit"),
    one("/t", EditOperation::Update)
  );
  let payload = json!({ "tool_input": { "file_path": "/f\n", "path": "/p" } });
  assert_eq!(
    normalize_edit_records(&payload, "Write"),
    one("/f", EditOperation::Write)
  );
}

#[test]
fn unreadable_payloads_are_malformed() {
  for payload in [
    json!(null),
    json!([1]),
    json!("x"),
    json!({ "tool_input": 3 }),
    json!({ "tool_input": { "file_path": "" } }),
    json!({ "tool_input": { "file_path": 1 } }),
    json!({ "tool_input": null }),
  ] {
    assert_eq!(
      normalize_edit_records(&payload, "Edit"),
      EditRecords::Malformed,
      "{payload}"
    );
  }
  let patch = json!({ "tool_input": { "command": 5 } });
  assert_eq!(
    normalize_edit_records(&patch, "apply_patch"),
    EditRecords::Malformed
  );
}

#[test]
fn records_print_one_compact_object_per_line() {
  let moved = vec![
    EditRecord {
      path: "a".into(),
      operation: EditOperation::Update,
      moved_to: Some("b".into()),
      from: None,
    },
    EditRecord {
      path: "b".into(),
      operation: EditOperation::Move,
      moved_to: None,
      from: Some("a".into()),
    },
  ];
  let text = "{\"path\":\"a\",\"operation\":\"update\",\"moved_to\":\"b\"}\n{\"path\":\"b\",\"operation\":\"move\",\"from\":\"a\"}\n";
  assert_eq!(format_edit_records(&moved), text);
  assert_eq!(format_edit_records(&[]), "");
  let names: Vec<&str> = [
    EditOperation::Add,
    EditOperation::Delete,
    EditOperation::Write,
  ]
  .into_iter()
  .map(EditOperation::name)
  .collect();
  assert_eq!(names, ["add", "delete", "write"]);
}
