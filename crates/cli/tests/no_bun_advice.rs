//! Nothing asks for Bun (#440): every plugin's `SessionStart` and
//! `UserPromptSubmit` entry, run through its real generated `hooks.json`
//! launcher with the built binary and a `PATH` without Bun, prints no word of
//! it, and neither does `toolu doctor --json`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

/// The `toolu` binary under test.
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A helper's result; the tests unwrap it.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// A `PATH` a host may give its hooks, with no Bun on it.
const PATH: &str = "/usr/bin:/bin";

fn repo() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `(plugin, event, command)` for every entry of `events` in every `hooks.json`.
fn entries(events: &[&str]) -> Res<Vec<(String, String, String)>> {
  let mut found = Vec::new();
  for dir in std::fs::read_dir(repo().join("plugins"))? {
    let dir = dir?.path();
    let Ok(text) = std::fs::read_to_string(dir.join("hooks/hooks.json")) else {
      continue;
    };
    let plugin = dir
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or("plugin name")?
      .to_owned();
    let doc: Value = serde_json::from_str(&text)?;
    for event in events {
      let groups = doc["hooks"][*event].as_array().cloned().unwrap_or_default();
      for hook in groups
        .iter()
        .flat_map(|group| group["hooks"].as_array().cloned().unwrap_or_default())
      {
        let command = hook["command"].as_str().ok_or("command")?.to_owned();
        found.push((plugin.clone(), (*event).to_owned(), command));
      }
    }
  }
  found.sort();
  Ok(found)
}

fn payload(event: &str) -> &'static str {
  if event == "SessionStart" {
    r#"{"hook_event_name":"SessionStart","source":"startup"}"#
  } else {
    r#"{"hook_event_name":"UserPromptSubmit","prompt":"rank these approaches"}"#
  }
}

/// Run one launcher command in a fresh home; stdout and stderr together.
fn launch(home: &Path, plugin: &str, event: &str, command: &str) -> Res<String> {
  let mut child = Command::new("/bin/sh")
    .args(["-c", command])
    .env_clear()
    .env("HOME", home)
    .env("PATH", PATH)
    .env("TOOLU_BIN", TOOLU)
    .env("CLAUDE_PLUGIN_ROOT", repo().join("plugins").join(plugin))
    .current_dir(home)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  if let Some(mut stdin) = child.stdin.take() {
    use std::io::Write as _;
    let _ignored = stdin.write_all(payload(event).as_bytes());
  }
  let output = child.wait_with_output()?;
  if output.status.code() != Some(0) {
    return Err(
      format!(
        "{plugin} {event} exited {:?}: {output:?}",
        output.status.code()
      )
      .into(),
    );
  }
  Ok(format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  ))
}

#[test]
fn no_session_or_prompt_hook_mentions_bun() {
  let found = entries(&["SessionStart", "UserPromptSubmit"]).unwrap();
  assert!(found.len() >= 10, "{found:?}");
  let home = tempfile::tempdir().unwrap();
  let mut all = String::new();
  for (plugin, event, command) in &found {
    let said = launch(home.path(), plugin, event, command).unwrap();
    assert!(
      !said.to_lowercase().contains("bun"),
      "{plugin} {event} mentions Bun: {said}"
    );
    all.push_str(&said);
  }
  assert!(all.contains("you MUST call toolu jev"), "the hooks ran: {all}");
  assert!(all.contains("toolu runtime: native"), "the hooks ran: {all}");
}

#[test]
fn doctor_has_no_bun_check() {
  let home = tempfile::tempdir().unwrap();
  let output = Command::new(TOOLU)
    .args(["--json", "doctor"])
    .env_clear()
    .env("HOME", home.path())
    .env("PATH", PATH)
    .current_dir(home.path())
    .output()
    .unwrap();
  let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
  let checks = doc["checks"].as_array().unwrap();
  assert!(!checks.is_empty());
  assert!(checks.iter().all(|check| check["id"] != "runtime"), "{doc}");
  assert!(!doc.to_string().to_lowercase().contains("bun"), "{doc}");
}
