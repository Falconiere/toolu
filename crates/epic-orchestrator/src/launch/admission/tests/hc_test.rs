use super::*;
use crate::herdr::Client as HerdrClient;
use crate::paths::Paths;
use serde_json::json;
use toolu_runtime::env::Env;

#[test]
fn refuses_session_change_before_touching_terminal() {
  let env = Env::from_pairs([("HC_SESSION", "other")]);
  let paths = Paths::at("/tmp");
  let herdr = Herdr {
    client: HerdrClient::new(),
    paths: &paths,
    env: &env,
  };
  let record = json!({"hc_session":"saved"}).as_object().unwrap().clone();
  assert!(
    stop(&record, &herdr)
      .unwrap_err()
      .message
      .contains("session differs")
  );
}
