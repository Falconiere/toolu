use toolu_runtime::env::Env;

use super::{herdr_reachable, session_name, socket_path, tmux_available};

#[test]
fn the_socket_path_honors_the_env_override() {
  let env = Env::from_pairs([("HERDR_SOCKET_PATH", "/tmp/custom.sock")]);
  assert_eq!(socket_path(&env).to_str(), Some("/tmp/custom.sock"));
}

#[test]
fn the_default_socket_lives_under_the_config_dir() {
  let env = Env::from_pairs([("HOME", "/home/user")]);
  assert_eq!(
    socket_path(&env).to_str(),
    Some("/home/user/.config/herdr/herdr.sock")
  );
}

#[test]
fn a_named_session_nests_the_socket() {
  let env = Env::from_pairs([("HOME", "/home/user"), ("HERDR_SESSION", "work")]);
  assert_eq!(
    socket_path(&env).to_str(),
    Some("/home/user/.config/herdr/sessions/work/herdr.sock")
  );
}

#[test]
fn the_session_name_prefers_the_toolu_var() {
  let env = Env::from_pairs([
    ("TOOLU_EPIC_HERDR_SESSION", "toolu"),
    ("HERDR_SESSION", "other"),
  ]);
  assert_eq!(session_name(&env), Some("toolu"));
  let fallback = Env::from_pairs([("HERDR_SESSION", "other")]);
  assert_eq!(session_name(&fallback), Some("other"));
}

#[test]
fn reachability_follows_the_socket() {
  let dir = tempfile::tempdir().unwrap();
  let sock = dir.path().join("herdr.sock");
  let env = Env::from_pairs([("HERDR_SOCKET_PATH", sock.to_str().unwrap())]);
  assert!(!herdr_reachable(&env));
  std::fs::write(&sock, "").unwrap();
  assert!(herdr_reachable(&env));
}

#[test]
fn the_config_path_parent_replaces_the_default_dir() {
  let env = Env::from_pairs([("HERDR_CONFIG_PATH", "/tmp/herdr/config.json")]);
  assert_eq!(socket_path(&env).to_str(), Some("/tmp/herdr/herdr.sock"));
}

#[test]
fn a_root_config_path_uses_the_current_dir() {
  let env = Env::from_pairs([("HERDR_CONFIG_PATH", "/")]);
  assert_eq!(socket_path(&env).to_str(), Some("./herdr.sock"));
}

#[test]
fn tmux_availability_is_a_path_scan() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("tmux"), "").unwrap();
  let env = Env::from_pairs([("PATH", dir.path().to_str().unwrap())]);
  assert!(tmux_available(&env));
  let empty = Env::from_pairs([("PATH", "/nonexistent")]);
  assert!(!tmux_available(&empty));
}
