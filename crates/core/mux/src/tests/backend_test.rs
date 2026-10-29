use toolu_runtime::config::multiplexer::Multiplexer;
use toolu_runtime::env::Env;

use super::{Backend, available, resolve};

fn env(path: &str) -> Env {
  Env::from_pairs([("PATH", path), ("HOME", "/nonexistent-home")])
}

#[test]
fn the_backend_name_matches_the_config_spelling() {
  assert_eq!(Backend::Herdr.name(), "herdr");
  assert_eq!(Backend::Tmux.name(), "tmux");
}

#[test]
fn a_pinned_backend_is_returned_verbatim() {
  assert_eq!(resolve(Multiplexer::Herdr, &env("/bin")), Backend::Herdr);
  assert_eq!(resolve(Multiplexer::Tmux, &env("/bin")), Backend::Tmux);
}

#[test]
fn auto_prefers_herdr_when_its_socket_exists() {
  let dir = tempfile::tempdir().unwrap();
  let sock = dir.path().join("herdr.sock");
  std::fs::write(&sock, "").unwrap();
  let env = Env::from_pairs([
    ("PATH", "/bin"),
    ("HOME", "/nonexistent-home"),
    ("HERDR_SOCKET_PATH", sock.to_str().unwrap()),
  ]);
  assert_eq!(resolve(Multiplexer::Auto, &env), Backend::Herdr);
}

#[test]
fn auto_falls_back_to_tmux_without_herdr() {
  let env = Env::from_pairs([
    ("PATH", "/bin"),
    ("HOME", "/nonexistent-home"),
    ("HERDR_SOCKET_PATH", "/nonexistent/herdr.sock"),
  ]);
  assert_eq!(resolve(Multiplexer::Auto, &env), Backend::Tmux);
}

#[test]
fn availability_tracks_the_backend() {
  let dir = tempfile::tempdir().unwrap();
  let sock = dir.path().join("herdr.sock");
  std::fs::write(&sock, "").unwrap();
  let herdr_env = Env::from_pairs([
    ("PATH", "/nonexistent"),
    ("HOME", "/nonexistent-home"),
    ("HERDR_SOCKET_PATH", sock.to_str().unwrap()),
  ]);
  assert!(available(Backend::Herdr, &herdr_env));
  assert!(!available(Backend::Tmux, &herdr_env));

  let missing = Env::from_pairs([
    ("PATH", "/bin"),
    ("HOME", "/nonexistent-home"),
    ("HERDR_SOCKET_PATH", "/nonexistent/herdr.sock"),
  ]);
  assert!(!available(Backend::Herdr, &missing));
}

#[test]
fn hc_context_selects_app_even_when_unavailable() {
  let env = env("/nonexistent").with("HC_TERMINAL_ID", "t7");
  assert_eq!(resolve(Multiplexer::Auto, &env), Backend::HerdrApp);
  assert_eq!(resolve(Multiplexer::HerdrApp, &env), Backend::HerdrApp);
  assert!(!available(Backend::HerdrApp, &env));
  assert_eq!(Backend::HerdrApp.name(), "herdr-app");
}
