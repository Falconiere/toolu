//! `toolu doctor` checks the same non-login shell that agent command tools use.

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

const CLEAR: &[&str] = &[
  "TOOLU_CONFIG_DIR",
  "TOOLU_USER_CONFIG_DIR",
  "CLAUDE_CONFIG_DIR",
  "CLAUDE_PROJECT_DIR",
  "CLAUDE_PLUGINS_REGISTRY",
  "CODEX_HOME",
  "CURSOR_PROJECT_DIR",
  "HERMES_HOME",
  "XDG_CONFIG_HOME",
  "TOOLU_OPENCODE_HOME",
  "TOOLU_CODEX_PLUGIN_SNAPSHOT",
  "TOOLU_BUN",
  "TOOLU_EPIC_TOKEN",
  "TOOLU_EPIC_STATUS_TOKEN",
  "TOOLU_EPIC_PEER_TOKENS",
  "TOOLU_EPIC_NOTIFY_URL",
];

fn doctor(home: &Path, path: &str, json_out: bool) -> Command {
  let mut command = Command::new(TOOLU);
  command.arg("--host").arg("claude");
  if json_out {
    command.arg("--json");
  }
  command
    .arg("doctor")
    .env("HOME", home)
    .env("PATH", path)
    .env("TOOLU_PROJECT_DIR", home);
  for key in CLEAR {
    command.env_remove(key);
  }
  command
}

fn copy_toolu(dir: &Path) -> std::io::Result<PathBuf> {
  let path = dir.join("toolu");
  std::fs::copy(TOOLU, &path).map(|_| path)
}

fn schema_accepts(document: &Value) -> bool {
  let schema_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.schema.json");
  let Ok(text) = std::fs::read_to_string(schema_file) else {
    return false;
  };
  let Ok(schema) = serde_json::from_str::<Value>(&text) else {
    return false;
  };
  let (Some(schema_id), Some(defs)) = (schema.get("$schema"), schema.get("$defs")) else {
    return false;
  };
  let doctor_schema = json!({
    "$schema": schema_id,
    "$defs": defs,
    "$ref": "#/$defs/doctor",
  });
  jsonschema::validator_for(&doctor_schema).is_ok_and(|validator| validator.is_valid(document))
}

#[test]
fn doctor_finds_the_real_binary_that_sh_resolves() {
  let temp = tempfile::tempdir().unwrap();
  let home = temp.path().join("home");
  std::fs::create_dir(&home).unwrap();
  let alias = temp.path().join("alias");
  symlink(temp.path(), &alias).unwrap();
  let path = copy_toolu(&alias).unwrap();
  let canonical = path.canonicalize().unwrap();
  let shell_path = format!("{}:/usr/bin:/bin", alias.display());
  let shell = Command::new("sh")
    .args(["-c", "command -v toolu"])
    .env("PATH", &shell_path)
    .output()
    .unwrap();
  assert_eq!(shell.status.code(), Some(0));
  assert_eq!(
    String::from_utf8_lossy(&shell.stdout).trim(),
    path.display().to_string()
  );
  let human = doctor(&home, &shell_path, false).output().unwrap();
  assert_eq!(human.status.code(), Some(0), "{human:?}");
  assert!(String::from_utf8_lossy(&human.stdout).contains(&canonical.display().to_string()));
  let json = doctor(&home, &shell_path, true).output().unwrap();
  assert_eq!(json.status.code(), Some(0), "{json:?}");
  let document: Value = serde_json::from_slice(&json.stdout).unwrap();
  assert_eq!(document["namespace"], "doctor");
  assert_eq!(document["ok"], true);
  assert_eq!(document["checks"][1]["id"], "reachability");
  assert_eq!(document["checks"][1]["details"]["reachable"], true);
  assert_eq!(
    document["checks"][1]["details"]["path"],
    canonical.display().to_string()
  );
  assert!(schema_accepts(&document), "{document}");
  let reduced = doctor(&home, &alias.display().to_string(), false)
    .output()
    .unwrap();
  assert_eq!(reduced.status.code(), Some(0), "{reduced:?}");
  assert!(String::from_utf8_lossy(&reduced.stdout).contains(&canonical.display().to_string()));
}

#[test]
fn doctor_prints_brew_upgrade_only_for_a_cellar_binary() {
  let temp = tempfile::tempdir().unwrap();
  let home = temp.path().join("home");
  std::fs::create_dir(&home).unwrap();
  let cellar = temp.path().join("Cellar/toolu/9.0.0/bin");
  let plain = temp.path().join("plain/bin");
  for (dir, brew) in [(&cellar, true), (&plain, false)] {
    std::fs::create_dir_all(dir).unwrap();
    copy_toolu(dir).unwrap();
    let output = doctor(&home, &format!("{}:/usr/bin:/bin", dir.display()), false)
      .output()
      .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
      stdout.contains("upgrade with: brew upgrade toolu"),
      brew,
      "{stdout}"
    );
    assert_eq!(
      stdout.contains("upgrade with: curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash"),
      !brew,
      "{stdout}"
    );
  }
}

#[test]
fn doctor_fails_when_the_non_login_shell_has_no_toolu() {
  let home = tempfile::tempdir().unwrap();
  let missing = doctor(home.path(), "/usr/bin:/bin", false)
    .output()
    .unwrap();
  assert_eq!(missing.status.code(), Some(1));
  let stdout = String::from_utf8_lossy(&missing.stdout);
  assert!(stdout.contains("non-login shell"), "{stdout}");
  assert!(String::from_utf8_lossy(&missing.stderr).contains("failed"));
  let json = doctor(home.path(), "/usr/bin:/bin", true).output().unwrap();
  assert_eq!(json.status.code(), Some(1));
  let document: Value = serde_json::from_slice(&json.stdout).unwrap();
  assert!(document.get("error").is_none(), "{document}");
  assert_eq!(document["ok"], false);
  assert_eq!(document["checks"][1]["details"]["reachable"], false);
  assert!(document["checks"][1]["details"]["path"].is_null());
  assert_eq!(document["checks"][1]["details"]["shadowed"], false);
}

#[test]
fn doctor_bounds_a_hanging_native_probe() {
  let temp = tempfile::tempdir().unwrap();
  let home = temp.path().join("home");
  std::fs::create_dir(&home).unwrap();
  let path = temp.path().join("toolu");
  std::fs::write(&path, "#!/bin/sh\nwhile :; do :; done\n").unwrap();
  let mut mode = std::fs::metadata(&path).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&path, mode).unwrap();
  let mut doctor = doctor(&home, temp.path().to_str().unwrap(), false)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
  let started = Instant::now();
  loop {
    if let Some(status) = doctor.try_wait().unwrap() {
      assert_eq!(status.code(), Some(1));
      break;
    }
    if started.elapsed() > Duration::from_secs(5) {
      doctor.kill().unwrap();
      doctor.wait().unwrap();
      panic!("doctor did not time out its native probe");
    }
    sleep(Duration::from_millis(20));
  }
}

#[test]
fn doctor_does_not_wait_for_a_descendant_holding_protocol_output_open() {
  let temp = tempfile::tempdir().unwrap();
  let home = temp.path().join("home");
  std::fs::create_dir(&home).unwrap();
  let path = temp.path().join("toolu");
  std::fs::write(&path, "#!/bin/sh\n/bin/sleep 3 &\nprintf '1\\n'\n").unwrap();
  let mut mode = std::fs::metadata(&path).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&path, mode).unwrap();
  let shell_path = format!("{}:/usr/bin:/bin", temp.path().display());
  let started = Instant::now();
  let output = doctor(&home, &shell_path, false).output().unwrap();
  assert_eq!(output.status.code(), Some(0), "{output:?}");
  assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn doctor_rejects_the_real_npm_wrapper_first_on_path() {
  let temp = tempfile::tempdir().unwrap();
  let home = temp.path().join("home");
  std::fs::create_dir(&home).unwrap();
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let wrapper = temp.path().join("toolu");
  let built = Command::new("bun")
    .args([
      "build",
      "tools/toolu-cli/src/cli.ts",
      "--target=node",
      "--outfile",
    ])
    .arg(&wrapper)
    .current_dir(repo)
    .output()
    .unwrap();
  assert_eq!(built.status.code(), Some(0), "{built:?}");
  let mut mode = std::fs::metadata(&wrapper).unwrap().permissions();
  mode.set_mode(0o755);
  std::fs::set_permissions(&wrapper, mode).unwrap();
  let node = Command::new("sh")
    .args(["-c", "command -v node"])
    .output()
    .unwrap();
  assert_eq!(node.status.code(), Some(0));
  let node = PathBuf::from(String::from_utf8(node.stdout).unwrap().trim());
  let node_dir = node.parent().unwrap();
  let shell_path = format!(
    "{}:{}:/usr/bin:/bin",
    temp.path().display(),
    node_dir.display()
  );
  let output = doctor(&home, &shell_path, true).output().unwrap();
  assert_eq!(output.status.code(), Some(1), "{output:?}");
  let document: Value = serde_json::from_slice(&output.stdout).unwrap();
  assert!(document.get("error").is_none(), "{document}");
  assert_eq!(document["checks"][1]["details"]["shadowed"], true);
  assert_eq!(document["checks"][1]["details"]["reachable"], false);
}
