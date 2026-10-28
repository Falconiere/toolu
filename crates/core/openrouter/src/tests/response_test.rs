use serde_json::json;

use super::{Error, parse};

#[test]
fn malformed_or_embedded_errors_never_become_clean() {
  assert_eq!(parse(b"garbage", "model"), Err(Error::InvalidResponse));
  assert_eq!(
    parse(br#"{"choices":[]}"#, "model"),
    Err(Error::InvalidResponse)
  );
  let embedded = json!({"choices":[{"message":{"content":"partial"},"finish_reason":"error","error":{"code":502,"message":"provider down"}}]}).to_string();
  assert_eq!(
    parse(embedded.as_bytes(), "model"),
    Err(Error::Upstream(502))
  );
  let top = json!({"error":{"code":429,"message":"busy"}}).to_string();
  assert_eq!(
    parse(top.as_bytes(), "model"),
    Err(Error::RateLimited {
      retry_after_secs: None
    })
  );
}

#[test]
fn empty_content_keeps_finish_reason_even_without_usage() {
  let body = br#"{"choices":[{"message":{"content":""},"finish_reason":"stop"}]}"#;
  assert_eq!(
    parse(body, "model"),
    Err(Error::Empty {
      finish_reason: Some("stop".into()),
      model: "model".into(),
      usage: None,
    })
  );
}

#[test]
fn null_content_is_a_typed_truncation_or_empty_error() {
  let length = br#"{"choices":[{"message":{"content":null},"finish_reason":"length"}]}"#;
  assert_eq!(
    parse(length, "model"),
    Err(Error::Truncated {
      finish_reason: Some("length".into()),
      model: "model".into(),
      usage: None,
    })
  );
  let stop = br#"{"choices":[{"message":{"content":null},"finish_reason":"stop"}]}"#;
  assert_eq!(
    parse(stop, "model"),
    Err(Error::Empty {
      finish_reason: Some("stop".into()),
      model: "model".into(),
      usage: None,
    })
  );
}

#[test]
fn a_null_error_field_is_success_and_the_served_model_is_recorded() {
  let body = json!({"error":null,"model":"served/model","choices":[{"error":null,"message":{"content":"{}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}).to_string();
  let completion = parse(body.as_bytes(), "requested/model").expect("completion");
  assert_eq!(completion.model, "served/model");
}

#[test]
fn completion_debug_includes_usage_but_never_source() {
  let content = "private-source-sentinel";
  let body = json!({"choices":[{"message":{"content":content},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":11,"cost":0.02},"provider":"Parasail"}).to_string();
  let completion = parse(body.as_bytes(), "requested/model").expect("completion");
  let shown = format!("{completion:?}");
  assert!(shown.contains("prompt_tokens: 7"));
  assert!(shown.contains("requested/model"));
  assert!(shown.contains("Parasail"));
  assert!(!shown.contains(content));
}
