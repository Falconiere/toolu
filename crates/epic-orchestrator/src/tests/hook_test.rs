use toolu_runtime::env::Env;

use super::{check_binary, check_deps, engine_ensure};

fn env(dir: &std::path::Path) -> Env {
  let path = |name: &str| dir.join(name).display().to_string();
  Env::from_pairs([
    ("HOME", path("home")),
    ("TOOLU_CONFIG_DIR", path("config")),
    ("TOOLU_RESOURCE_HOME", path("resources")),
    ("PATH", path("empty")),
  ])
}

#[test]
fn engine_ensure_is_silent_without_epics_and_reports_a_malformed_registry() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  assert_eq!(engine_ensure(&env).stdout, None);
  let resources = tmp.path().join("resources");
  std::fs::create_dir_all(&resources).expect("resources");
  std::fs::write(
    resources.join("registry.json"),
    r#"{"version":1,"epics":[]}"#,
  )
  .expect("empty");
  assert_eq!(engine_ensure(&env).stdout, None);
  std::fs::write(resources.join("registry.json"), "{not json").expect("malformed");
  let outcome = engine_ensure(&env);
  let doc: serde_json::Value =
    serde_json::from_str(outcome.stdout.as_deref().expect("message")).expect("json");
  let message = doc["systemMessage"].as_str().expect("system message");
  assert!(message.starts_with("epic engine: "), "{message}");
  assert!(message.contains("registry.json"), "{message}");
  assert!(message.chars().count() <= "epic engine: ".len() + 200);
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Success);
}

#[test]
fn check_deps_is_silent_off_codex_and_check_binary_advises_once() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env(tmp.path());
  assert_eq!(check_deps(&env).stdout, None);
  let payload = Some(r#"{"session_id":"epic"}"#);
  assert!(check_binary(&env, payload).stdout.is_some());
  assert_eq!(check_binary(&env, payload).stdout, None);
}
