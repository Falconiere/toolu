//! Secret values stay out of `toolu config get` and `toolu doctor`.

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
const CANARY: &str = "CANARYVALUE9f3a2c7e";
const TOKEN: &str = "TOKENVALUE88aa77";

const CLEAR: &str = "TOOLU_CONFIG_DIR TOOLU_USER_CONFIG_DIR CLAUDE_CONFIG_DIR CLAUDE_PROJECT_DIR \
CODEX_HOME CURSOR_PROJECT_DIR HERMES_HOME XDG_CONFIG_HOME TOOLU_OPENCODE_HOME \
TOOLU_EPIC_TOKEN TOOLU_EPIC_STATUS_TOKEN TOOLU_EPIC_PEER_TOKENS TOOLU_EPIC_NOTIFY_URL";

struct Sandbox {
  home: PathBuf,
  project: PathBuf,
}

fn sandbox() -> Option<Sandbox> {
  let root = tempfile::tempdir().ok()?.into_path();
  let home = root.join("home");
  let project = root.join("project");
  std::fs::create_dir_all(home.join(".claude/toolu")).ok()?;
  std::fs::create_dir_all(project.join(".claude")).ok()?;
  Some(Sandbox { home, project })
}

fn toolu(box_: &Sandbox, args: &[&str]) -> Option<std::process::Output> {
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
  command.output().ok()
}

fn write_secrets(box_: &Sandbox, mode: u32) -> bool {
  let path = box_.home.join(".claude/toolu/secrets.json");
  let body = format!(r#"{{"version":1,"status_token":"{CANARY}"}}"#);
  if std::fs::write(&path, body).is_err() {
    return false;
  }
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).is_ok()
}

fn write_config(box_: &Sandbox) -> bool {
  let body = format!(
    r#"{{"version":1,"epic":{{"label":"prefix {CANARY} suffix","statusToken":"{TOKEN}"}}}}"#
  );
  std::fs::write(box_.home.join(".claude/toolu.config.json"), body).is_ok()
}

fn text(output: &std::process::Output) -> String {
  format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  )
}

#[test]
fn loaded_secrets_are_redacted_in_get_and_doctor() {
  let box_ = sandbox().expect("sandbox");
  assert!(write_secrets(&box_, 0o600));
  assert!(write_config(&box_));
  for args in [
    &["config", "get"][..],
    &["--json", "config", "get"][..],
    &["doctor"][..],
    &["--json", "doctor"][..],
  ] {
    let Some(output) = toolu(&box_, args) else {
      panic!("spawn");
    };
    let shown = text(&output);
    assert!(!shown.contains(CANARY), "{args:?}\n{shown}");
    assert!(!shown.contains(TOKEN), "{args:?}\n{shown}");
  }
  let Some(get) = toolu(&box_, &["config", "get"]) else {
    panic!("spawn");
  };
  assert!(text(&get).contains("<redacted>"), "{}", text(&get));
  let Some(doctor) = toolu(&box_, &["--json", "doctor"]) else {
    panic!("spawn");
  };
  let document: Value =
    serde_json::from_str(&String::from_utf8_lossy(&doctor.stdout)).expect("json");
  let checks = document["checks"].as_array().expect("checks");
  let config = checks.iter().find(|check| check["id"] == "config");
  let Some(config) = config else {
    panic!("config check");
  };
  let summary = config["summary"].as_str().unwrap_or("");
  assert!(
    summary.contains("epic.statusToken: credential belongs in toolu/secrets.json"),
    "{summary}"
  );
  assert_eq!(
    config["details"]["merged"]["epic"]["statusToken"],
    "<redacted>"
  );
  assert!(
    !config["details"]["merged"].to_string().contains(CANARY),
    "{}",
    config["details"]["merged"]
  );
}

#[test]
fn unsafe_secrets_omit_the_document_and_the_canary() {
  let box_ = sandbox().expect("sandbox");
  assert!(write_secrets(&box_, 0o644));
  assert!(write_config(&box_));
  let Some(get) = toolu(&box_, &["config", "get"]) else {
    panic!("spawn");
  };
  assert_eq!(get.status.code(), Some(1));
  assert_eq!(get.stdout, Vec::<u8>::new());
  let shown = text(&get);
  assert!(!shown.contains(CANARY), "{shown}");
  assert!(!shown.contains(TOKEN), "{shown}");
  let Some(doctor) = toolu(&box_, &["--json", "doctor"]) else {
    panic!("spawn");
  };
  let doctor_text = text(&doctor);
  assert!(!doctor_text.contains(CANARY), "{doctor_text}");
  assert!(!doctor_text.contains(TOKEN), "{doctor_text}");
  let document: Value =
    serde_json::from_str(&String::from_utf8_lossy(&doctor.stdout)).expect("json");
  let checks = document["checks"].as_array().expect("checks");
  let config = checks.iter().find(|check| check["id"] == "config");
  let Some(config) = config else {
    panic!("config check");
  };
  assert!(config["details"].get("merged").is_none());
  assert_eq!(config["status"], "fail");
}
