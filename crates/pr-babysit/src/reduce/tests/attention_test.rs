use serde_json::{Value, json};

use super::Work;

fn work<'a>(threads: &'a [Value], escalations: &'a [Value]) -> Work<'a> {
  Work {
    escalations,
    ci: "pass",
    checks: &[],
    head: "abc",
    threads,
    stale: &[],
    conversation: &[],
    reviews: &[],
    review_changes: false,
    provider_error: false,
    bot_run: "1@t".into(),
    fixer: None,
  }
}

#[test]
fn the_fingerprint_follows_the_reply_target_not_the_tick() {
  let one = [json!({"id": "PRRT_a", "inReplyTo": 7})];
  let first = work(&one, &[]).attention(None);
  assert_eq!(first["codes"], json!(["threads_actionable"]));
  let print = first["fingerprint"].as_str().unwrap();
  assert_eq!(work(&one, &[]).attention(Some(print))["new"], false);
  let follow_up = [json!({"id": "PRRT_a", "inReplyTo": 8})];
  let next = work(&follow_up, &[]).attention(Some(print));
  assert_eq!(next["new"], true);
  assert_ne!(next["fingerprint"], first["fingerprint"]);
}

#[test]
fn nothing_to_do_has_no_fingerprint_and_escalations_come_first() {
  let none = work(&[], &[]).attention(Some("old"));
  assert_eq!(
    none,
    json!({"needed": false, "new": false, "codes": [], "fingerprint": null})
  );
  let escalations = [json!({"code": "merge_conflict", "detail": "x"})];
  let threads = [json!({"id": "t", "inReplyTo": 1})];
  let both = work(&threads, &escalations).attention(None);
  assert_eq!(both["codes"], json!(["escalate", "threads_actionable"]));
}
