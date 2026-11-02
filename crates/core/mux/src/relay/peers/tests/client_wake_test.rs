use super::{enroll, owns_host};
use serde_json::json;
use toolu_runtime::env::Env;

#[test]
fn only_a_real_foreground_ancestor_owns_the_host() {
  let env = Env::from_pairs([("PATH", "/usr/bin:/bin")]);
  let pid = std::process::id();
  let exe = std::env::current_exe().unwrap();
  let host = exe.file_name().unwrap().to_str().unwrap();
  assert!(owns_host(
    &env,
    &json!({"foreground":{"pid":pid,"argv":[exe]}}),
    host
  ));
  assert!(!owns_host(
    &env,
    &json!({"foreground":{"pid":pid,"argv":[exe]}}),
    "unrelated"
  ));
  let parent = std::process::Command::new("ps")
    .args(["-o", "ppid=", "-p", &pid.to_string()])
    .output()
    .unwrap();
  assert!(parent.status.success());
  let parent_id: u32 = String::from_utf8(parent.stdout)
    .unwrap()
    .trim()
    .parse()
    .unwrap();
  let command = std::process::Command::new("ps")
    .args(["-o", "comm=", "-p", &parent_id.to_string()])
    .output()
    .unwrap();
  assert!(command.status.success());
  let command = String::from_utf8(command.stdout).unwrap();
  let parent_exe = command.trim();
  let parent_host = std::path::Path::new(parent_exe)
    .file_name()
    .unwrap()
    .to_str()
    .unwrap();
  assert!(owns_host(
    &env,
    &json!({"foreground":{"pid":parent_id,"argv":[parent_exe]}}),
    parent_host
  ));
  let mut child = std::process::Command::new("true").spawn().unwrap();
  let other = child.id();
  assert!(child.wait().unwrap().success());
  assert!(!owns_host(
    &env,
    &json!({"foreground":{"pid":other,"argv":["true"]}}),
    "true"
  ));
}

#[test]
fn unverified_terminal_never_changes_session_state() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().to_str().unwrap();
  let state = dir.path().join("delegation/peers/clients/alice/state.json");
  std::fs::create_dir_all(state.parent().unwrap()).unwrap();
  let original = r#"{"version":1,"session":{"id":"alice","generation":"gen","machine":"one","host":"codex","group":"team","active":true,"wake":null},"secret":"secret","relay":"relay","authority":"foundation","group_key":"key"}"#;
  std::fs::write(&state, original).unwrap();
  let base = Env::from_pairs([("TOOLU_CONFIG_DIR", root), ("PATH", "/definitely/absent")]);
  enroll(&base, "alice", "one", "codex").unwrap();
  for terminal in ["../bad", "t", "t1"] {
    enroll(
      &base.clone().with("HC_TERMINAL_ID", terminal),
      "alice",
      "one",
      "codex",
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&state).unwrap(), original);
  }
}
