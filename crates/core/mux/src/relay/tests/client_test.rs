use super::*;

#[test]
fn missing_relay_fails_without_local_fallback() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  assert!(
    call(&env, &json!({"op":"list"}))
      .unwrap_err()
      .contains("relay is not configured")
  );
}
