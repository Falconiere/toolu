//! Real-binary status token rotation and output secrecy.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn secret_file(root: &Path) -> std::path::PathBuf {
  root.join("toolu/secrets.json")
}

fn command(root: &Path, json: bool, env_token: Option<&str>) -> std::io::Result<Output> {
  let mut command = Command::new(TOOLU);
  command
    .env_remove("TOOLU_EPIC_STATUS_TOKEN")
    .env_remove("TOOLU_EPIC_TOKEN")
    .args(["--host", "codex", "--config-dir"])
    .arg(root)
    .args(["epic", "token", "new"]);
  if json {
    command.arg("--json");
  }
  if let Some(token) = env_token {
    command.env("TOOLU_EPIC_STATUS_TOKEN", token);
  }
  command.output()
}

fn file_value(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
  let bytes = std::fs::read(secret_file(root))?;
  Ok(serde_json::from_slice(&bytes)?)
}

#[test]
fn rotation_preserves_other_fields_and_never_prints_a_token() {
  let root = tempfile::tempdir().unwrap();
  let path = secret_file(root.path());
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, r#"{"version":1,"status_token":"old-canary","notify_url":"keep-notify","peer_tokens":{"alpha":"keep-peer"}}"#).unwrap();
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

  let first = command(root.path(), false, None).unwrap();
  assert_eq!(
    first.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&first.stderr)
  );
  let file = file_value(root.path()).unwrap();
  let first_token = file["status_token"].as_str().unwrap().to_owned();
  assert_eq!(first_token.len(), 64);
  assert_eq!(file["notify_url"], "keep-notify");
  assert_eq!(file["peer_tokens"]["alpha"], "keep-peer");
  assert!(!String::from_utf8_lossy(&first.stdout).contains(&first_token));

  let second = command(root.path(), true, None).unwrap();
  assert_eq!(
    second.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&second.stderr)
  );
  let file = file_value(root.path()).unwrap();
  let second_token = file["status_token"].as_str().unwrap();
  assert_ne!(second_token, first_token);
  let response: Value = serde_json::from_slice(&second.stdout).unwrap();
  assert_eq!(response["status_token"], "<redacted>");
  let shown = format!(
    "{}{}",
    String::from_utf8_lossy(&second.stdout),
    String::from_utf8_lossy(&second.stderr)
  );
  for token in [
    first_token.as_str(),
    second_token,
    "keep-notify",
    "keep-peer",
  ] {
    assert!(!shown.contains(token));
  }
  assert_eq!(
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
    0o600
  );
}

#[test]
fn unsafe_file_and_active_environment_override_refuse_rotation() {
  let root = tempfile::tempdir().unwrap();
  let path = secret_file(root.path());
  assert_eq!(
    command(root.path(), false, None).unwrap().status.code(),
    Some(0)
  );
  let before = std::fs::read(&path).unwrap();

  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
  let unsafe_file = command(root.path(), true, None).unwrap();
  assert_eq!(unsafe_file.status.code(), Some(1));
  assert!(String::from_utf8_lossy(&unsafe_file.stderr).contains("chmod 600"));
  assert_eq!(std::fs::read(&path).unwrap(), before);

  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
  let overridden = command(root.path(), true, Some("CANARY_ENV_TOKEN")).unwrap();
  assert_eq!(overridden.status.code(), Some(1));
  let shown = format!(
    "{}{}",
    String::from_utf8_lossy(&overridden.stdout),
    String::from_utf8_lossy(&overridden.stderr)
  );
  assert!(shown.contains("TOOLU_EPIC_STATUS_TOKEN"));
  assert!(!shown.contains("CANARY_ENV_TOKEN"));
  assert_eq!(std::fs::read(path).unwrap(), before);
}
