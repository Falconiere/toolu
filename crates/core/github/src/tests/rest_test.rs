use serde_json::Value;

use crate::{Error, Fresh};

#[test]
fn a_fresh_body_decodes_or_names_the_failure() {
  let fresh = Fresh {
    status: 200,
    etag: Some("\"v1\"".into()),
    headers: Vec::new(),
    body: br#"{"number":460}"#.to_vec(),
  };
  assert_eq!(fresh.json::<Value>().expect("json")["number"], 460);
  let broken = Fresh {
    body: b"{".to_vec(),
    ..fresh
  };
  assert!(matches!(broken.json::<Value>(), Err(Error::Decode(_))));
}

#[test]
fn a_decode_error_does_not_quote_the_body() {
  let fresh = Fresh {
    status: 200,
    etag: None,
    headers: Vec::new(),
    body: br#""token-like-value""#.to_vec(),
  };
  let Err(Error::Decode(message)) = fresh.json::<u64>() else {
    panic!("expected a decode error");
  };
  assert_eq!(message, "unexpected data error at line 1 column 18");
}
