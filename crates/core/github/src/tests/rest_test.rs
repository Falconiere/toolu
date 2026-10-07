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
