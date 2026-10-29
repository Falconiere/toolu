use super::{Intent, validate_target};
use crate::writes::Target;

#[test]
fn an_intent_preserves_its_original_thread_root_and_refuses_a_changed_one() {
  let original = Target::Thread {
    thread: "PRRT_a",
    root: 41,
    in_reply_to: 42,
  };
  let intent = Intent::new("Falconiere/toolu", 165, &original, "fixed", "now").unwrap();
  let restored = Intent::read(&intent.value()).unwrap();
  assert_eq!(restored.original_body().unwrap(), "fixed");
  assert_eq!(restored.root_comment_id, Some(41));
  assert_eq!(
    restored.post_path,
    "/repos/Falconiere/toolu/pulls/165/comments/41/replies"
  );
  assert!(validate_target("thread:PRRT_a@42", &restored, &original).is_ok());
  let changed = Target::Thread {
    thread: "PRRT_a",
    root: 99,
    in_reply_to: 42,
  };
  assert_eq!(
    validate_target("thread:PRRT_a@42", &restored, &changed)
      .unwrap_err()
      .code,
    "slot_mismatch"
  );
}

#[test]
fn an_intent_with_a_body_missing_its_marker_is_malformed() {
  let to = Target::Conversation(5);
  let intent = Intent::new("Falconiere/toolu", 165, &to, "fixed", "now").unwrap();
  let mut value = intent.value();
  value["body"] = "fixed".into();
  let restored = Intent::read(&value).unwrap();
  assert_eq!(
    restored.original_body().unwrap_err().code,
    "state_malformed"
  );
}

#[test]
fn an_empty_body_is_rejected_before_an_intent_can_be_saved() {
  let to = Target::Conversation(5);
  assert_eq!(
    Intent::new("Falconiere/toolu", 165, &to, "", "now")
      .err()
      .unwrap()
      .code,
    "usage"
  );
}
