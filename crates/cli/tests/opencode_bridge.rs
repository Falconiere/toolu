//! `toolu opencode bridge` and `catalog` through the real binary: an `OpenCode`
//! hook call against the repository's own plugin directories, each hook run as
//! the binary itself with a Claude-shaped payload.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn plugin(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

struct Sb {
  dir: tempfile::TempDir,
}

impl Sb {
  fn new() -> Res<Sb> {
    let dir = tempfile::tempdir()?;
    for sub in ["home", "project", "settings"] {
      fs::create_dir_all(dir.path().join(sub))?;
    }
    fs::write(dir.path().join("settings/protected-files.txt"), ".env\n")?;
    Ok(Sb { dir })
  }

  fn path(&self, sub: &str) -> PathBuf {
    self.dir.path().join(sub)
  }

  fn run(&self, args: &[&str], stdin: &str) -> Res<Output> {
    let mut command = assert_cmd::Command::new(TOOLU);
    command
      .args(args)
      .env_clear()
      .env("PATH", std::env::var("PATH")?)
      .env("HOME", self.path("home"))
      .env("TOOLU_SETTINGS_DIR", self.path("settings"))
      .current_dir(self.path("project"))
      .write_stdin(stdin.to_owned());
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }

  fn bridge(&self, hook: &str, plugin_name: &str, event: &Value) -> Res<Value> {
    let root = plugin(plugin_name).display().to_string();
    let call = json!({"directory": self.path("project"), "event": event}).to_string();
    let args = [
      "opencode",
      "bridge",
      "--event",
      hook,
      "--plugin-root",
      &root,
    ];
    let out = self.run(&args, &call)?;
    if out.status.code() != Some(0) {
      return Err(String::from_utf8_lossy(&out.stderr).into_owned().into());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
  }
}

#[test]
fn a_protected_file_write_is_denied_before_the_tool_runs() {
  let sb = Sb::new().unwrap();
  let target = sb.path("project/.env").display().to_string();
  let event = json!({"tool": "write", "sessionID": "ses_1", "id": "call_1",
    "input": {"path": target, "content": "SECRET=1"}});
  let reply = sb.bridge("tool.execute.before", "toolu", &event).unwrap();
  assert_eq!(reply["action"], "deny", "{reply}");
  assert!(
    reply["reason"].as_str().unwrap().contains(".env"),
    "{reply}"
  );
}

#[test]
fn a_shell_write_to_a_protected_file_is_denied_and_a_read_runs() {
  let sb = Sb::new().unwrap();
  let shell = json!({"tool": "shell", "sessionID": "s", "id": "c",
    "input": {"command": "echo SECRET=1 >.env"}});
  let reply = sb.bridge("tool.execute.before", "toolu", &shell).unwrap();
  assert_eq!(reply["action"], "deny", "{reply}");
  let read = json!({"tool": "read", "sessionID": "s", "id": "c", "input": {"path": "a.txt"}});
  assert_eq!(
    sb.bridge("tool.execute.before", "toolu", &read).unwrap(),
    json!({"action": "continue"})
  );
}

#[test]
fn session_start_runs_every_session_entry_without_refusing() {
  let sb = Sb::new().unwrap();
  let reply = sb
    .bridge("session.start", "toolu", &json!({"sessionID": "ses_1"}))
    .unwrap();
  assert_eq!(reply["action"], "continue", "{reply}");
  let said = format!("{} {}", reply["message"], reply["context"]);
  assert!(said.contains("toolu runtime: native"), "{reply}");
}

#[test]
fn a_bad_call_fails_closed_before_a_tool() {
  let sb = Sb::new().unwrap();
  let root = plugin("toolu").display().to_string();
  let args = [
    "opencode",
    "bridge",
    "--event",
    "tool.execute.before",
    "--plugin-root",
    &root,
  ];
  let out = sb.run(&args, "not json").unwrap();
  assert_eq!(out.status.code(), Some(0));
  let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
  assert_eq!(reply["action"], "deny");
  assert!(reply["reason"].as_str().unwrap().contains("not JSON"));
}

#[test]
fn catalog_names_skills_and_commands_for_the_shim() {
  let sb = Sb::new().unwrap();
  let root = plugin("pr-babysit").display().to_string();
  let out = sb
    .run(&["opencode", "catalog", "--plugin-root", &root], "")
    .unwrap();
  assert_eq!(out.status.code(), Some(0));
  let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
  assert_eq!(doc["skills"][0]["id"], "pr-babysit:babysit");
  assert_eq!(doc["commands"][0]["name"], "pr-babysit:babysit");
  let template = doc["commands"][0]["template"].as_str().unwrap();
  assert!(template.contains(&format!("{root}/workflows/babysit.md")));
}
