//! `toolu config` get, set and validate against isolated config files.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

const CLEAR: &str = "TOOLU_CONFIG_DIR TOOLU_USER_CONFIG_DIR CLAUDE_CONFIG_DIR CLAUDE_PROJECT_DIR \
CODEX_HOME CURSOR_PROJECT_DIR HERMES_HOME XDG_CONFIG_HOME TOOLU_OPENCODE_HOME \
TOOLU_EPIC_TOKEN TOOLU_EPIC_STATUS_TOKEN TOOLU_EPIC_PEER_TOKENS TOOLU_EPIC_NOTIFY_URL";

struct Out {
  code: i32,
  stdout: String,
  stderr: String,
}

struct Sandbox {
  home: PathBuf,
  project: PathBuf,
}

fn sandbox() -> Option<Sandbox> {
  let root = tempfile::tempdir().ok()?.into_path();
  let home = root.join("home");
  let project = root.join("project");
  std::fs::create_dir_all(home.join(".claude")).ok()?;
  std::fs::create_dir_all(project.join(".claude")).ok()?;
  Some(Sandbox { home, project })
}

fn toolu(box_: &Sandbox, args: &[&str]) -> Out {
  let mut command = Command::new(TOOLU);
  command
    .args(["--host", "claude"])
    .args(args)
    .env("HOME", &box_.home)
    .env("TOOLU_PROJECT_DIR", &box_.project)
    .env("PATH", "/usr/bin:/bin");
  for key in CLEAR.split(' ') {
    command.env_remove(key);
  }
  let Ok(output) = command.output() else {
    return Out {
      code: 127,
      stdout: String::new(),
      stderr: String::new(),
    };
  };
  Out {
    code: output.status.code().unwrap_or(127),
    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
  }
}

fn user_file(box_: &Sandbox) -> PathBuf {
  box_.home.join(".claude/toolu.config.json")
}

fn project_file(box_: &Sandbox) -> PathBuf {
  box_.project.join(".claude/toolu.config.json")
}

fn write(path: &Path, body: &str) -> bool {
  std::fs::write(path, body).is_ok()
}

fn bytes(path: &Path) -> Vec<u8> {
  std::fs::read(path).unwrap_or_default()
}

#[test]
fn an_unknown_key_fails_validate_with_the_loader_message() {
  let box_ = sandbox().expect("sandbox");
  let path = project_file(&box_);
  assert!(write(&path, "{\"bogus\":1}\n"));
  let out = toolu(&box_, &["config", "validate"]);
  assert_eq!(out.code, 1, "{}", out.stderr);
  assert!(
    out.stderr.contains("unknown top-level key 'bogus'"),
    "{}",
    out.stderr
  );
  assert!(out.stdout.is_empty(), "{}", out.stdout);
  let json = toolu(&box_, &["--json", "config", "validate"]);
  assert_eq!(json.code, 1);
  let document: Value = serde_json::from_str(json.stdout.trim()).expect("json");
  assert_eq!(document["valid"], json!(false));
}

#[test]
fn refused_writes_leave_the_file_bytes_identical() {
  let box_ = sandbox().expect("sandbox");
  let path = user_file(&box_);
  assert!(write(&path, "{\"version\":1}\n"));
  let before = bytes(&path);
  let version = toolu(&box_, &["config", "set", "version", "2"]);
  assert_eq!(version.code, 1, "{}", version.stderr);
  assert_eq!(bytes(&path), before);
  let token = toolu(
    &box_,
    &["config", "set", "epic.statusToken", "\"secret-value\""],
  );
  assert_eq!(token.code, 1, "{}", token.stderr);
  assert!(!token.stderr.contains("secret-value"), "{}", token.stderr);
  assert_eq!(bytes(&path), before);
  assert!(write(&path, "{\"version\":1,\"gates\":\"no\"}\n"));
  let blocked = bytes(&path);
  let nested = toolu(&box_, &["config", "set", "gates.pushReview", "\"off\""]);
  assert_eq!(nested.code, 1, "{}", nested.stderr);
  assert_eq!(bytes(&path), blocked);
}

#[test]
fn set_creates_a_missing_object_and_get_round_trips() {
  let box_ = sandbox().expect("sandbox");
  let path = user_file(&box_);
  assert!(write(&path, "{\"version\":1}\n"));
  let set = toolu(&box_, &["config", "set", "gates.pushReview", "\"off\""]);
  assert_eq!(set.code, 0, "{}", set.stderr);
  let get = toolu(&box_, &["config", "get", "gates.pushReview"]);
  assert_eq!(get.code, 0, "{}", get.stderr);
  assert_eq!(get.stdout.trim(), "off");
  let missing = toolu(&box_, &["config", "get", "no.such"]);
  assert_eq!(missing.code, 1);
  assert!(missing.stdout.is_empty(), "{}", missing.stdout);
  assert_eq!(
    missing.stderr.trim(),
    "toolu config get: no such key 'no.such'"
  );
}

#[test]
fn a_missing_file_is_created_and_an_empty_segment_is_usage() {
  let box_ = sandbox().expect("sandbox");
  let path = user_file(&box_);
  assert!(!path.exists());
  let set = toolu(&box_, &["config", "set", "gates.pushReview", "\"off\""]);
  assert_eq!(set.code, 0, "{}", set.stderr);
  assert!(path.is_file());
  let get = toolu(&box_, &["--json", "config", "get", "gates.pushReview"]);
  assert_eq!(get.code, 0, "{}", get.stderr);
  let document: Value = serde_json::from_str(get.stdout.trim()).expect("json");
  assert_eq!(document["namespace"], json!("config"));
  assert_eq!(document["value"], json!("off"));
  let usage = toolu(&box_, &["config", "set", "gates.", "\"off\""]);
  assert_eq!(usage.code, 64, "{}", usage.stderr);
}
