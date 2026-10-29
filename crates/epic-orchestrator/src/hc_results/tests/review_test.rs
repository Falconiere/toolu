use serde_json::json;

use crate::launch::record::{Record, load_launch_record};

#[test]
fn handoff_intent_keeps_remote_identity_until_local_address_is_known() {
  let tmp = tempfile::tempdir().expect("temp");
  let file = tmp.path().join("issue.json");
  let mut record = Record::new();
  record.insert("terminal_address".into(), json!("peer/t7"));
  record.insert("terminal_name".into(), json!("remote"));
  record.insert("agent".into(), json!("hc:peer/t7"));
  record.insert("hc_run".into(), json!("0123456789abcdef0123456789abcdef"));
  super::prepare(&file, &mut record).expect("persist");
  let saved = load_launch_record(&file).expect("valid record");
  assert_eq!(saved["terminal_address"], "peer/t7");
  assert_eq!(saved["remote_terminal_address"], "peer/t7");
  assert_eq!(
    saved["review_terminal_name"],
    "review-0123456789abcdef01234567"
  );
  assert_eq!(saved["review_prompt_state"], "pending");
  assert!(!saved.contains_key("controller_review"));
}

#[test]
fn wait_envelope_and_live_terminal_must_both_report_idle() {
  let wait = json!({"status":"idle", "ended":false});
  let live = json!({"state":"running", "status":{"state":"idle"}});
  assert!(super::ready_to_send(&wait, &live));
  assert!(!super::ready_to_send(&json!({"status":"blocked"}), &live));
  assert!(!super::ready_to_send(
    &wait,
    &json!({"state":"finished", "status":{"state":"idle"}})
  ));
}
