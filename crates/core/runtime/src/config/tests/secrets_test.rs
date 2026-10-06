use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{load, path, redact_json, redact_text, rotate_status_token};
use crate::env::Env;
use crate::host::roots::Roots;

fn roots(dir: &Path) -> Roots {
  Roots::new(
    Env::from_pairs([
      ("HOME", dir.display().to_string()),
      ("TOOLU_CONFIG_DIR", dir.display().to_string()),
    ]),
    Some(Host::Codex),
  )
}

fn write(path: &Path, value: &Value, mode: u32) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, format!("{value}\n")).unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn unsafe_modes_symlink_and_malformed_bytes_fail_without_leaking() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  let secret_path = path(&roots);
  write(
    &secret_path,
    &json!({"version":1,"status_token":"canary-mode"}),
    0o644,
  );
  let error = load(&roots).unwrap_err().to_string();
  assert!(error.contains("chmod 600"), "{error}");
  assert!(!error.contains("canary-mode"));

  std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600)).unwrap();
  assert_eq!(load(&roots).unwrap().status_token(), Some("canary-mode"));

  let victim = dir.path().join("victim.json");
  std::fs::rename(&secret_path, &victim).unwrap();
  symlink(&victim, &secret_path).unwrap();
  assert!(load(&roots).is_err());
  std::fs::remove_file(&secret_path).unwrap();
  std::fs::write(&secret_path, b"{not-json-canary").unwrap();
  std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600)).unwrap();
  let error = load(&roots).unwrap_err().to_string();
  assert!(!error.contains("canary"), "{error}");
}

#[test]
fn env_overrides_file_per_field_and_rejects_bad_peer_json() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  write(
    &path(&roots),
    &json!({
      "version":1,"status_token":"file-status","notify_url":"file-notify",
      "peer_tokens":{"alpha":"file-alpha","beta":"file-beta"}
    }),
    0o600,
  );
  let env = roots
    .env()
    .clone()
    .with("TOOLU_EPIC_TOKEN", "old-alias")
    .with("TOOLU_EPIC_STATUS_TOKEN", "primary")
    .with("TOOLU_EPIC_NOTIFY_URL", "env-notify")
    .with("TOOLU_EPIC_PEER_TOKENS", r#"{"alpha":"env-alpha"}"#);
  let resolved = load(&Roots::new(env, Some(Host::Codex))).unwrap();
  assert_eq!(resolved.status_token(), Some("primary"));
  assert_eq!(resolved.notify_url(), Some("env-notify"));
  assert_eq!(resolved.peer_token("alpha"), Some("env-alpha"));
  assert_eq!(resolved.peer_token("beta"), Some("file-beta"));
  let shown = redact_text(
    "file-status file-notify file-alpha file-beta old-alias primary env-notify env-alpha",
    &resolved,
  );
  for secret in [
    "file-status",
    "file-notify",
    "file-alpha",
    "file-beta",
    "old-alias",
    "primary",
    "env-notify",
    "env-alpha",
  ] {
    assert!(!shown.contains(secret), "{shown}");
  }

  let alias = roots.env().clone().with("TOOLU_EPIC_TOKEN", "old-alias");
  assert_eq!(
    load(&Roots::new(alias, Some(Host::Codex)))
      .unwrap()
      .status_token(),
    Some("old-alias")
  );
  let bad = roots
    .env()
    .clone()
    .with("TOOLU_EPIC_PEER_TOKENS", r#"{"alpha":4}"#);
  let error = load(&Roots::new(bad, Some(Host::Codex)))
    .unwrap_err()
    .to_string();
  assert!(!error.contains("alpha"));
}

#[test]
fn rotation_is_atomic_and_preserves_other_fields() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  let secret_path = path(&roots);
  write(
    &secret_path,
    &json!({
      "version":1,"status_token":"old","notify_url":"keep-notify",
      "peer_tokens":{"alpha":"keep-peer"},"future":{"safe":true}
    }),
    0o600,
  );
  rotate_status_token(&roots).unwrap();
  let first = load(&roots).unwrap().status_token().unwrap().to_owned();
  assert_eq!(first.len(), 64);
  assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
  rotate_status_token(&roots).unwrap();
  let second = load(&roots).unwrap();
  assert_ne!(second.status_token(), Some(first.as_str()));
  assert_eq!(second.notify_url(), Some("keep-notify"));
  assert_eq!(second.peer_token("alpha"), Some("keep-peer"));
  let bytes: Value = serde_json::from_slice(&std::fs::read(&secret_path).unwrap()).unwrap();
  assert_eq!(bytes["future"], json!({"safe":true}));
  assert_eq!(
    std::fs::metadata(secret_path).unwrap().permissions().mode() & 0o777,
    0o600
  );
}

#[test]
fn rotation_refuses_an_active_status_environment_override() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  rotate_status_token(&roots).unwrap();
  let before = std::fs::read(path(&roots)).unwrap();
  for variable in ["TOOLU_EPIC_STATUS_TOKEN", "TOOLU_EPIC_TOKEN"] {
    let env = roots.env().clone().with(variable, "CANARY_ENV_TOKEN");
    let error = rotate_status_token(&Roots::new(env, Some(Host::Codex)))
      .unwrap_err()
      .to_string();
    assert!(error.contains(variable), "{error}");
    assert!(!error.contains("CANARY_ENV_TOKEN"));
    assert_eq!(std::fs::read(path(&roots)).unwrap(), before);
  }
}

#[test]
fn empty_environment_values_leave_file_credentials_active() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  write(
    &path(&roots),
    &json!({
      "version":1,"status_token":"file-status","notify_url":"file-notify",
      "peer_tokens":{"alpha":"file-alpha"}
    }),
    0o600,
  );
  let env = roots
    .env()
    .clone()
    .with("TOOLU_EPIC_STATUS_TOKEN", "")
    .with("TOOLU_EPIC_TOKEN", "")
    .with("TOOLU_EPIC_NOTIFY_URL", "")
    .with("TOOLU_EPIC_PEER_TOKENS", "");
  let empty_roots = Roots::new(env, Some(Host::Codex));
  let resolved = load(&empty_roots).unwrap();
  assert_eq!(resolved.status_token(), Some("file-status"));
  assert_eq!(resolved.notify_url(), Some("file-notify"));
  assert_eq!(resolved.peer_token("alpha"), Some("file-alpha"));
  rotate_status_token(&empty_roots).unwrap();
  assert_ne!(load(&roots).unwrap().status_token(), Some("file-status"));
}

#[test]
fn redacted_documents_and_debug_never_contain_canaries() {
  let dir = tempfile::tempdir().unwrap();
  let roots = roots(dir.path());
  write(
    &path(&roots),
    &json!({
      "version":1,"status_token":"CANARY_STATUS","notify_url":"https://host/CANARY_NOTIFY",
      "peer_tokens":{"peer":"CANARY_PEER"}
    }),
    0o600,
  );
  let secrets = load(&roots).unwrap();
  let documents = [
    json!({"config":{"status_token":"CANARY_STATUS","hint":"has CANARY_PEER"}}),
    json!({"doctor":{"notify_url":"https://host/CANARY_NOTIFY"}}),
    json!({"journal":{"message":"attempt CANARY_STATUS failed"}}),
    json!({"status":{"peer_token":"CANARY_PEER","note":"https://host/CANARY_NOTIFY"}}),
  ];
  for document in documents {
    let shown = redact_json(&document, &secrets).to_string();
    assert!(!shown.contains("CANARY_"), "{shown}");
    assert!(shown.contains("<redacted>"), "{shown}");
  }
  assert_eq!(
    redact_text("failed CANARY_STATUS", &secrets),
    "failed <redacted>"
  );
  assert!(!format!("{secrets:?}").contains("CANARY_"));
}
