//! Muse standalone hub hooks through the native `toolu` binary and captured payloads.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

struct Fixture {
  _temp: tempfile::TempDir,
  project: PathBuf,
  settings: PathBuf,
}

impl Fixture {
  fn new() -> io::Result<Self> {
    let temp = tempfile::tempdir()?;
    let project = temp.path().join("project");
    let settings = temp.path().join("settings");
    fs::create_dir_all(&project)?;
    fs::create_dir_all(&settings)?;
    let init = Command::new("git")
      .args(["init", "-q", "-b", "feat/x"])
      .current_dir(&project)
      .status()?;
    if !init.success() {
      return Err(io::Error::other("git init failed"));
    }
    let commit = Command::new("git")
      .args([
        "-c",
        "user.name=Test",
        "-c",
        "user.email=t@example.com",
        "commit",
        "--allow-empty",
        "-q",
        "-m",
        "base",
      ])
      .current_dir(&project)
      .status()?;
    if !commit.success() {
      return Err(io::Error::other("git commit failed"));
    }
    Ok(Self {
      _temp: temp,
      project,
      settings,
    })
  }

  fn hook(&self, name: &str, payload: &Value) -> io::Result<Output> {
    let path = std::env::var_os("PATH")
      .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PATH is not set"))?;
    let mut cmd = Command::new(TOOLU);
    cmd
      .args(["hook", name, "--event", "PreToolUse"])
      .current_dir(&self.project)
      .env_clear()
      .env("PATH", path)
      .env(
        "HOME",
        self
          .project
          .parent()
          .ok_or_else(|| io::Error::other("parent"))?,
      )
      .env("MUSE_PLUGIN_ID", "toolu")
      .env(
        "PLUGIN_ROOT",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu"),
      )
      .env("TOOLU_PROJECT_DIR", &self.project)
      .env(
        "TOOLU_CONFIG_DIR",
        self
          .project
          .parent()
          .ok_or_else(|| io::Error::other("parent"))?
          .join("config"),
      )
      .env("TOOLU_SETTINGS_DIR", &self.settings)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    for key in ["TMPDIR", "LLVM_PROFILE_FILE"] {
      if let Some(value) = std::env::var_os(key) {
        cmd.env(key, value);
      }
    }
    let mut child = cmd.spawn()?;
    child
      .stdin
      .take()
      .ok_or_else(|| io::Error::other("piped stdin missing"))?
      .write_all(payload.to_string().as_bytes())?;
    child.wait_with_output()
  }

  fn telemetry(&self) -> PathBuf {
    self.project.join(".toolu/tmp/muse/telemetry/feat_x.jsonl")
  }
}

fn captured(tool: &str) -> io::Result<Value> {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../docs/toolu/evidence/2026-10-19-muse-tool-inventory.json");
  let inventory: Value =
    serde_json::from_str(&fs::read_to_string(path)?).map_err(io::Error::other)?;
  inventory
    .get("tools")
    .and_then(|tools| tools.get(tool))
    .and_then(|entry| entry.get("pre_example"))
    .cloned()
    .ok_or_else(|| io::Error::other(format!("missing captured {tool} payload")))
}

fn silent(out: &Output) {
  assert_eq!(out.status.code(), Some(0), "{out:?}");
  assert!(out.stdout.is_empty(), "{out:?}");
  assert!(out.stderr.is_empty(), "{out:?}");
}

#[test]
fn muse_mcp_call_blocks_and_unmatched_or_internal_calls_are_silent() {
  let fixture = Fixture::new().expect("fixture");
  fs::write(
    fixture.settings.join("mcp-blocklist.txt"),
    "plugin_toolu_probe_540_probe_mcp -> use CLI\n",
  )
  .expect("blocklist");
  let mcp = captured("mcp__plugin_toolu_probe_540_probe_mcp__probe_echo").expect("MCP payload");
  let out = fixture.hook("mcp-tools", &mcp).expect("hook");
  assert_eq!(out.status.code(), Some(0), "{out:?}");
  assert!(out.stderr.is_empty(), "{out:?}");
  let decision: Value = serde_json::from_slice(&out.stdout).expect("Muse decision");
  assert_eq!(decision["decision"], "block");
  assert!(
    decision["reason"]
      .as_str()
      .expect("reason")
      .contains("use CLI")
  );
  assert!(decision.get("hookSpecificOutput").is_none());
  fs::write(fixture.settings.join("mcp-blocklist.txt"), "other_server\n")
    .expect("unmatched blocklist");
  silent(&fixture.hook("mcp-tools", &mcp).expect("hook"));
  let internal = captured("submit_reminder_decision").expect("internal payload");
  silent(&fixture.hook("mcp-tools", &internal).expect("hook"));
  silent(&fixture.hook("agent-tier", &internal).expect("hook"));
  assert!(!fixture.telemetry().exists());
}

#[test]
fn muse_spawn_records_observed_role_and_structured_child_fields() {
  let fixture = Fixture::new().expect("fixture");
  let mut spawn = captured("subagent_spawn").expect("spawn payload");
  silent(&fixture.hook("agent-tier", &spawn).expect("captured spawn"));
  let text = fs::read_to_string(fixture.telemetry()).expect("telemetry");
  let first: Value = serde_json::from_str(text.lines().next().expect("line")).expect("JSON");
  assert_eq!(first["subagent_type"], "responder");
  assert!(first["model"].is_null());
  assert!(first["reasoning_effort"].is_null());
  let ledger = fixture
    .project
    .join(".toolu/tmp/muse/plan-ledger/feat_x.json");
  fs::create_dir_all(ledger.parent().expect("parent")).expect("ledger dir");
  fs::write(
    ledger,
    r#"{"steps":[{"id":"s1","status":"running","model":"opus"}],"next":"s1"}"#,
  )
  .expect("ledger");
  spawn["tool_input"]["model"] = json!("sonnet");
  spawn["tool_input"]["effort"] = json!("high");
  let out = fixture.hook("agent-tier", &spawn).expect("extended spawn");
  assert_eq!(out.status.code(), Some(0), "{out:?}");
  assert!(out.stderr.is_empty(), "{out:?}");
  let advice: Value = serde_json::from_slice(&out.stdout).expect("Muse advice");
  assert!(
    advice["systemMessage"]
      .as_str()
      .expect("message")
      .contains("opus")
  );
  assert!(advice.get("hookSpecificOutput").is_none());
  let text = fs::read_to_string(fixture.telemetry()).expect("telemetry");
  let second: Value = serde_json::from_str(text.lines().nth(1).expect("line")).expect("JSON");
  assert_eq!(second["model"], "sonnet");
  assert_eq!(second["subagent_type"], "responder");
  assert_eq!(second["reasoning_effort"], "high");
}
