use toolu_state::edit_records::{EditOperation, EditRecord};

use super::{MALFORMED_PATCH_BLOCK, MALFORMED_PATCH_DENY, refusal, synthetic_edit};
use crate::dispatch::Phase;
use crate::dispatch::output::parse_document;

#[test]
fn a_split_path_is_an_edit_payload_in_javascript_key_order() {
  let doc = parse_document(
    r#"{"session_id":"s","tool_name":"apply_patch","tool_input":{"path":"old","command":"p","1":true},"cwd":"/p"}"#,
  )
  .unwrap();
  let record = EditRecord {
    path: "/p/new.ts".to_owned(),
    operation: EditOperation::Move,
    moved_to: None,
    from: Some("/p/old.ts".to_owned()),
  };
  assert_eq!(
    synthetic_edit(&doc, &record),
    r#"{"session_id":"s","tool_name":"Edit","tool_input":{"1":true,"path":"/p/new.ts","command":"p","file_path":"/p/new.ts","toolu_edit_operation":"move","toolu_edit_from":"/p/old.ts","toolu_edit_moved_to":""},"cwd":"/p"}"#
  );
  let bare = parse_document(r#"{"tool_name":"Write","tool_input":false}"#).unwrap();
  let write = EditRecord {
    path: "a".to_owned(),
    operation: EditOperation::Write,
    moved_to: None,
    from: None,
  };
  assert!(synthetic_edit(&bare, &write).contains(r#""tool_input":{"file_path":"a","path":"a""#));
}

#[test]
fn refusals_are_fixed_compact_lines() {
  assert_eq!(
    refusal(Phase::Post, "a \"quoted\" reason").stdout,
    "{\"decision\":\"block\",\"reason\":\"a \\\"quoted\\\" reason\"}\n"
  );
  assert!(
    refusal(Phase::Pre, "r")
      .stdout
      .ends_with("\"permissionDecisionReason\":\"r\"}}\n")
  );
  assert!(MALFORMED_PATCH_DENY.ends_with("}}\n") && MALFORMED_PATCH_BLOCK.ends_with("}\n"));
}
