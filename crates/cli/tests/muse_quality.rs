//! Muse quality registration retirement and push-time enforcement through the real CLI.

#[path = "helpers/feed.rs"]
mod feed;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

struct Fixture {
  _temp: tempfile::TempDir,
  project: PathBuf,
  config: PathBuf,
  report: PathBuf,
}

impl Fixture {
  fn new() -> io::Result<Self> {
    let temp = tempfile::tempdir()?;
    let project = temp.path().join("project");
    let config = temp.path().join("muse-config");
    let report = temp.path().join("startup.jsonl");
    fs::create_dir_all(project.join("src"))?;
    fs::create_dir_all(config.join("toolu/post-tools.d"))?;
    fs::create_dir_all(config.join("toolu/pre-tools.d"))?;
    fs::write(&report, "")?;
    fs::write(
      project.join("Cargo.toml"),
      "[package]\nname=\"demo\"\nversion=\"0.1.0\"\n",
    )?;
    let status = Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .status()?;
    if !status.success() {
      return Err(io::Error::other("git init failed"));
    }
    fs::write(
      config.join("toolu/post-tools.d/rust-quality@toolu__rust-quality.js"),
      "old",
    )?;
    fs::write(
      config.join("toolu/post-tools.d/rust-quality@toolu__rust-quality.json"),
      "{\"version\":1}",
    )?;
    fs::write(
      config.join("toolu/pre-tools.d/rust-quality@toolu__old.sh"),
      "old",
    )?;
    Ok(Self {
      _temp: temp,
      project,
      config,
      report,
    })
  }

  fn run(&self, args: &[&str], payload: &str) -> io::Result<Output> {
    let mut child = Command::new(TOOLU)
      .args(args)
      .current_dir(&self.project)
      .env("HOME", &self.project)
      .env("TOOLU_HOST_OVERRIDE", "muse")
      .env("MUSE_CONFIG_DIR", &self.config)
      .env("TOOLU_PROJECT_DIR", &self.project)
      .env("TOOLU_STARTUP_REPORT", &self.report)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()?;
    feed::feed(child.stdin.take(), payload.as_bytes())?;
    child.wait_with_output()
  }

  fn retire_rule(&self, plugin: &str) -> io::Result<()> {
    let root = plugin_root(plugin).display().to_string();
    let args = [
      plugin,
      "hook",
      "forget",
      "--event",
      "SessionStart",
      "--plugin-root",
      &root,
    ];
    let retired = self.run(&args, "{}")?;
    ensure(
      retired.status.code() == Some(0),
      format!("{plugin}: {retired:?}"),
    )?;
    for dir in ["post-tools.d", "pre-tools.d"] {
      let entries: Vec<_> = fs::read_dir(self.config.join("toolu").join(dir))?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(&format!("{plugin}@toolu__")))
        .collect();
      ensure(
        entries.is_empty(),
        format!("{plugin} left {entries:?} in {dir}"),
      )?;
    }
    let again = self.run(&args, "{}")?;
    ensure(again.status.code() == Some(0), format!("{again:?}"))?;
    let check = [
      plugin,
      "hook",
      "check-toolu",
      "--event",
      "SessionStart",
      "--plugin-root",
      &root,
    ];
    let checked = self.run(&check, "{}")?;
    ensure(
      (checked.status.code(), checked.stdout.len()) == (Some(0), 0),
      format!("check-toolu was not silent: {checked:?}"),
    )?;
    Ok(())
  }

  fn assert_cleanup_and_report(&self) -> io::Result<()> {
    let post = self.config.join("toolu/post-tools.d");
    let pre = self.config.join("toolu/pre-tools.d");
    ensure(
      !post.join("rust-quality@toolu__rust-quality.js").exists(),
      "old JavaScript module remains",
    )?;
    ensure(
      !post.join("rust-quality@toolu__rust-quality.json").exists(),
      "earlier rule manifest remains",
    )?;
    ensure(
      !pre.join("rust-quality@toolu__old.sh").exists(),
      "old shell bridge remains",
    )?;
    let lines = fs::read_to_string(&self.report)?;
    ensure(
      lines.is_empty(),
      format!("forget published something: {lines}"),
    )?;
    Ok(())
  }

  fn assert_red_gate_blocks_push_not_commit(&self) -> io::Result<()> {
    let state = self
      .project
      .join(".toolu/tmp/muse/quality-gate-status.json");
    fs::create_dir_all(
      state
        .parent()
        .ok_or_else(|| io::Error::other("no state dir"))?,
    )?;
    fs::write(
      &state,
      "{\"status\": \"failing\", \"errors\": [\"cargo test\"]}",
    )?;
    let push = json!({"session_id":"s","cwd":self.project,"tool_name":"bash",
      "tool_input":{"command":"git push origin main"}});
    let denied = self.run(
      &["hook", "pre-tools", "--event", "PreToolUse"],
      &push.to_string(),
    )?;
    ensure(denied.status.code() == Some(0), format!("{denied:?}"))?;
    let decision: Value = serde_json::from_slice(&denied.stdout).map_err(io::Error::other)?;
    ensure(
      decision.get("decision").and_then(Value::as_str) == Some("block"),
      format!("push was not blocked: {decision}"),
    )?;
    let commit = json!({"session_id":"s","cwd":self.project,"tool_name":"bash",
      "tool_input":{"command":"git commit -m test"}});
    let allowed = self.run(
      &["hook", "pre-tools", "--event", "PreToolUse"],
      &commit.to_string(),
    )?;
    ensure(allowed.status.code() == Some(0), format!("{allowed:?}"))?;
    let decision: Value = serde_json::from_slice(&allowed.stdout).map_err(io::Error::other)?;
    ensure(
      decision.get("decision").is_none(),
      format!("commit was blocked: {decision}"),
    )?;
    Ok(())
  }
}

fn plugin_root(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

fn ensure(condition: bool, detail: impl Into<String>) -> io::Result<()> {
  if condition {
    Ok(())
  } else {
    Err(io::Error::other(detail.into()))
  }
}

#[test]
fn muse_retires_quality_registration_and_blocks_a_red_push() {
  let fixture = Fixture::new().unwrap();
  for plugin in ["ts-quality", "python-quality", "rust-quality"] {
    fixture.retire_rule(plugin).unwrap();
  }
  fixture.assert_cleanup_and_report().unwrap();
  fixture.assert_red_gate_blocks_push_not_commit().unwrap();
}
