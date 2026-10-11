//! Native statusline settings, migration and compatibility through real files.

#[macro_use]
#[path = "helpers/ensure.rs"]
mod ensure;
#[path = "helpers/statusline.rs"]
mod statusline;

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;
use statusline::{TOOLU, TestResult, cmd, isolated_env, text};

#[test]
fn setup_targets_claude_even_with_another_host_selected() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let codex = temp.path().join("codex");
  let mut command = Command::new(TOOLU);
  command
    .args(["--host", "codex", "statusline", "setup"])
    .env_clear()
    .env("HOME", temp.path())
    .env("CODEX_HOME", &codex)
    .env(
      "TMPDIR",
      std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned()),
    );
  if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let output = command.output()?;
  ensure!(output.status.success());
  ensure!(!codex.join("settings.json").exists());
  let settings: Value =
    serde_json::from_slice(&fs::read(temp.path().join(".claude/settings.json"))?)?;
  ensure_eq!(settings["statusLine"]["command"], "toolu statusline render");
  Ok(())
}

#[test]
fn setup_refuses_custom_settings_then_force_backs_them_up() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  fs::create_dir_all(&cfg)?;
  let path = cfg.join("settings.json");
  let original = r#"{"statusLine":{"type":"command","command":"custom-bar"},"theme":"dark"}"#;
  fs::write(&path, original)?;
  let refused = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "setup"],
    None,
  )?;
  ensure_eq!(refused.status.code(), Some(2));
  ensure!(text(&refused).starts_with("REFUSED "));
  ensure!(!cfg.join("settings.json.bak").exists());
  let forced = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["--json", "statusline", "setup", "--force"],
    None,
  )?;
  ensure!(forced.status.success());
  let doc: Value = serde_json::from_slice(&forced.stdout)?;
  ensure_eq!(doc["status"], "WIRED");
  ensure_eq!(fs::read_to_string(cfg.join("settings.json.bak"))?, original);
  let written: Value = serde_json::from_slice(&fs::read(path)?)?;
  ensure_eq!(written["theme"], "dark");
  ensure_eq!(written["statusLine"]["command"], "toolu statusline render");
  Ok(())
}

#[test]
fn settings_create_and_idempotence() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  fs::create_dir_all(&cfg)?;
  let path = cfg.join("settings.json");
  let first = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "setup"],
    None,
  )?;
  ensure!(text(&first).starts_with("CREATED "));
  let before = fs::read(&path)?;
  let second = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "setup"],
    None,
  )?;
  ensure!(text(&second).starts_with("ALREADY "));
  ensure_eq!(fs::read(&path)?, before);
  ensure!(!cfg.join("settings.json.bak").exists());
  Ok(())
}

#[test]
fn settings_bad_documents_and_lookalike_paths_are_refused() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  fs::create_dir_all(&cfg)?;
  let path = cfg.join("settings.json");
  for (body, prefix, code) in [
    ("{", "ERROR ", 1),
    ("[]", "ERROR ", 1),
    (
      r#"{"statusLine":{"type":"command","command":"custom | cat"}}"#,
      "REFUSED ",
      2,
    ),
    (
      r#"{"statusLine":{"type":"command","command":"/tmp/other/statusline/statusline.sh"}}"#,
      "REFUSED ",
      2,
    ),
    (
      r#"{"statusLine":{"type":"command","command":"bash custom-program ~/.claude/statusline/statusline.sh"}}"#,
      "REFUSED ",
      2,
    ),
  ] {
    fs::write(&path, body)?;
    let output = cmd(
      temp.path(),
      &cfg,
      temp.path(),
      &["statusline", "setup"],
      None,
    )?;
    ensure_eq!(output.status.code(), Some(code));
    ensure!(text(&output).starts_with(prefix));
    ensure_eq!(fs::read_to_string(&path)?, body);
  }
  Ok(())
}

#[test]
fn settings_preserve_other_keys_and_upgrade_known_legacy_paths() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  fs::create_dir_all(&cfg)?;
  let path = cfg.join("settings.json");
  let original = r#"{"theme":"dark"}"#;
  fs::write(&path, original)?;
  let preserved = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "setup"],
    None,
  )?;
  ensure!(text(&preserved).starts_with("WIRED "));
  let doc: Value = serde_json::from_slice(&fs::read(&path)?)?;
  ensure_eq!(doc["theme"], "dark");
  ensure_eq!(fs::read_to_string(cfg.join("settings.json.bak"))?, original);
  let legacy =
    r#"{"statusLine":{"type":"command","command":"/bin/bash ~/.claude/statusline/statusline.sh"}}"#;
  fs::write(&path, legacy)?;
  let migrated = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "setup"],
    None,
  )?;
  ensure!(text(&migrated).starts_with("WIRED "));
  let doc: Value = serde_json::from_slice(&fs::read(&path)?)?;
  ensure_eq!(doc["statusLine"]["command"], "toolu statusline render");
  ensure_eq!(fs::read_to_string(cfg.join("settings.json.bak"))?, legacy);
  Ok(())
}

fn shim_line(cfg: &Path, cwd: &Path) -> TestResult<String> {
  let linked = cfg.join("statusline/statusline.sh");
  let bin_dir = Path::new(TOOLU)
    .parent()
    .ok_or_else(|| std::io::Error::other("binary parent"))?;
  let mut shim = Command::new(linked);
  isolated_env(&mut shim, cwd, cfg);
  let mut shim = shim
    .current_dir(cwd)
    .env("PATH", format!("{}:/usr/bin:/bin", bin_dir.display()))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
  shim
    .stdin
    .take()
    .ok_or_else(|| std::io::Error::other("shim stdin"))?
    .write_all(b"{}")?;
  let output = shim.wait_with_output()?;
  ensure!(output.status.success());
  Ok(text(&output))
}

#[test]
fn session_start_migrates_once_and_the_published_shim_runs_the_renderer() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  fs::create_dir_all(&cfg)?;
  let settings = cfg.join("settings.json");
  fs::write(
    &settings,
    r#"{"statusLine":{"type":"command","command":"bash ~/.claude/statusline/statusline.sh"}}"#,
  )?;
  let args = [
    "statusline",
    "hook",
    "session-start",
    "--event",
    "SessionStart",
  ];
  let first = cmd(temp.path(), &cfg, temp.path(), &args, Some("{}"))?;
  ensure!(
    first.status.success(),
    "{}",
    String::from_utf8_lossy(&first.stderr)
  );
  ensure!(text(&first).contains("migrated"));
  let second = cmd(temp.path(), &cfg, temp.path(), &args, Some("{}"))?;
  ensure!(second.status.success());
  ensure_eq!(text(&second), "");
  ensure!(cfg.join("statusline/statusline.sh").is_symlink());
  let migrated: Value = serde_json::from_slice(&fs::read(&settings)?)?;
  ensure_eq!(migrated["statusLine"]["command"], "toolu statusline render");
  ensure!(cfg.join("settings.json.bak").exists());
  let line = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "render"],
    Some("{}"),
  )?;
  ensure_eq!(shim_line(&cfg, temp.path())?, text(&line));
  Ok(())
}

#[test]
fn session_start_keeps_custom_settings_and_a_user_owned_compatibility_file() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  let legacy_dir = cfg.join("statusline");
  fs::create_dir_all(&legacy_dir)?;
  let path = cfg.join("settings.json");
  let original = r#"{"theme":"dark","statusLine":{"type":"command","command":"my-status"}}"#;
  fs::write(&path, original)?;
  let owned = legacy_dir.join("statusline.sh");
  fs::write(&owned, "#!/bin/sh\nexit 0\n")?;
  let hook = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &[
      "statusline",
      "hook",
      "session-start",
      "--event",
      "SessionStart",
    ],
    Some("{}"),
  )?;
  ensure!(hook.status.success());
  ensure_eq!(text(&hook), "");
  ensure_eq!(fs::read_to_string(&path)?, original);
  ensure!(!cfg.join("settings.json.bak").exists());
  ensure_eq!(fs::read_to_string(owned)?, "#!/bin/sh\nexit 0\n");
  Ok(())
}

#[test]
fn refresh_reports_json_and_is_idempotent() -> TestResult<()> {
  let temp = tempfile::tempdir()?;
  let cfg = temp.path().join("cfg");
  fs::create_dir_all(&cfg)?;
  fs::write(
    cfg.join("settings.json"),
    r#"{"statusLine":{"type":"command","command":"~/.claude/statusline/statusline.sh"}}"#,
  )?;
  let args = ["--json", "statusline", "refresh"];
  let first = cmd(temp.path(), &cfg, temp.path(), &args, None)?;
  ensure!(first.status.success());
  let value: Value = serde_json::from_slice(&first.stdout)?;
  ensure_eq!(value["status"], "WIRED");
  ensure_eq!(value["published"], true);
  ensure_eq!(value["migrated"], true);
  let second = cmd(temp.path(), &cfg, temp.path(), &args, None)?;
  let value: Value = serde_json::from_slice(&second.stdout)?;
  ensure_eq!(value["status"], "REFRESHED");
  ensure_eq!(value["migrated"], false);
  let text_mode = cmd(
    temp.path(),
    &cfg,
    temp.path(),
    &["statusline", "refresh"],
    None,
  )?;
  ensure!(text(&text_mode).starts_with("REFRESHED "));
  Ok(())
}
