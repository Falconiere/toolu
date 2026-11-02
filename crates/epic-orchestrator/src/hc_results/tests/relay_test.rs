use serde_json::Map;
use toolu_runtime::env::Env;

#[test]
fn result_without_durable_request_fails_before_contacting_relay() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("HOME", dir.path().to_str().unwrap())]);
  let error = super::result(&dir.path().join("issue.json"), &mut Map::new(), &env).unwrap_err();
  assert_eq!(error, "missing relay request");
}
