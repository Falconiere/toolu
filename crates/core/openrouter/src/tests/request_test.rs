use serde_json::{Value, json};

use super::{Message, StructuredRequest};
use crate::Error;

#[test]
fn malformed_request_fields_are_rejected_before_network_work() {
  let schema = json!({"type":"object"});
  let messages = [Message {
    role: "user",
    content: "x",
  }];
  let mut request = StructuredRequest {
    messages: &messages,
    schema_name: "review",
    schema: &schema,
    reasoning_effort: "high",
    max_tokens: 100,
  };
  let encoded: Value =
    serde_json::from_slice(&request.encode("model").expect("encode")).expect("body");
  assert_eq!(encoded["response_format"]["type"], "json_schema");
  assert_eq!(encoded["provider"]["zdr"], true);
  assert!(encoded.get("temperature").is_none());
  request.max_tokens = 0;
  assert_eq!(request.encode("model"), Err(Error::InvalidConfig));
  request.max_tokens = 100;
  request.reasoning_effort = "none";
  assert_eq!(request.encode("model"), Err(Error::InvalidConfig));
}
