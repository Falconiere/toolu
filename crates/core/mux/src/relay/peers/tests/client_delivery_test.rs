use super::{super::client_model::Outgoing, apply_response};
use serde_json::{Value, json};

fn queued() -> Outgoing {
  let envelope = json!({"version":1,"sender":{"id":"alice","generation":"gen"},"recipient":{"id":"bob","generation":"gen"},"group":"team","conversation_id":"m1","message_id":"m1","reply_to":null,"timestamp":123,"text":"hello"});
  serde_json::from_value(json!({"envelope":envelope,"delivery":"queued","receipt":{"outbox":true,"inbox":false,"acknowledged":false}})).unwrap()
}

#[test]
fn invalid_or_mismatched_responses_never_fabricate_delivery() {
  let item = queued();
  let envelope = serde_json::to_value(&item.envelope).unwrap();
  for response in [
    json!({}),
    json!({"version":2,"result":{"key":"alice/m1","envelope":envelope,"acknowledged":true,"wake":"unsupported"}}),
    json!({"version":1,"result":{"key":"other/m1","envelope":envelope,"acknowledged":true,"wake":"unsupported"}}),
    json!({"version":1,"result":{"key":"alice/m1","envelope":{},"acknowledged":true,"wake":"unsupported"}}),
    json!({"version":1}),
  ] {
    let mut pending = item.clone();
    apply_response(&mut pending, "alice", "m1", response).unwrap();
    assert_eq!(pending.delivery, "queued");
    assert!(!pending.receipt.inbox);
    assert!(!pending.receipt.acknowledged);
    assert_eq!(pending.error, Some(json!("invalid peer response")));
    assert!(pending.message.is_none());
  }
}

#[test]
fn retryable_errors_remain_queued_and_permanent_errors_are_rejected() {
  for (code, retryable, expected) in [
    ("storage", true, "queued"),
    ("unauthorized", false, "rejected"),
    ("stale", false, "rejected"),
  ] {
    let mut item = queued();
    let error = json!({"code":code,"message":"failure","retryable":retryable});
    apply_response(&mut item, "alice", "m1", json!({"version":1,"error":error})).unwrap();
    assert_eq!(item.delivery, expected);
    assert_eq!(item.error, Some(error));
    assert!(!item.receipt.inbox);
    assert!(!item.receipt.acknowledged);
  }
}

#[test]
fn remote_receipts_preserve_immutable_outbox_content() {
  let envelope = json!({"version":1,"sender":{"id":"alice","generation":"gen"},"recipient":{"id":"bob","generation":"gen"},"group":"team","conversation_id":"m1","message_id":"m1","reply_to":null,"timestamp":123,"text":"hello"});
  let mut item: Outgoing = serde_json::from_value(json!({"envelope":envelope,"delivery":"queued","receipt":{"outbox":true,"inbox":false,"acknowledged":false},"error":"offline"})).unwrap();
  let mut message =
    json!({"key":"alice/m1","envelope":envelope,"acknowledged":false,"wake":"unsupported"});
  apply_response(
    &mut item,
    "alice",
    "m1",
    json!({"version":1,"result":message}),
  )
  .unwrap();
  let value: Value = serde_json::to_value(&item).unwrap();
  assert_eq!(value["envelope"], envelope);
  assert_eq!(value["delivery"], "delivered");
  assert_eq!(
    value["receipt"],
    json!({"outbox":true,"inbox":true,"acknowledged":false})
  );
  assert!(value.get("error").is_none());
  message["acknowledged"] = json!(true);
  apply_response(
    &mut item,
    "alice",
    "m1",
    json!({"version":1,"result":message}),
  )
  .unwrap();
  let value: Value = serde_json::to_value(&item).unwrap();
  assert_eq!(value["delivery"], "acknowledged");
  assert_eq!(
    value["receipt"],
    json!({"outbox":true,"inbox":true,"acknowledged":true})
  );
  assert_eq!(value["envelope"], envelope);
  assert_eq!(value["message"], message);
}
