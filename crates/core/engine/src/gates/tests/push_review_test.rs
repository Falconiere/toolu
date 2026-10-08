use super::*;

#[test]
fn missing_state_message_includes_atomic_schema_instructions() {
  let reason = no_state_reason("abc", "main", "/tmp/review.json");
  assert!(reason.contains("atomically write /tmp/review.json"));
  assert!(reason.contains("reviewed_files"));
}
