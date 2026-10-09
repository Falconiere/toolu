use toolu_runtime::env::Env;

use super::session_id;

#[test]
fn hook_payload_session_id_wins_over_environment() {
  let env = Env::from_pairs([("TOOLU_SESSION_ID", "fallback")]);
  assert_eq!(
    session_id(&env, Some(r#"{"session_id":"payload"}"#)),
    Some("payload".to_owned())
  );
  assert_eq!(
    session_id(&env, Some("invalid")),
    Some("fallback".to_owned())
  );
}
