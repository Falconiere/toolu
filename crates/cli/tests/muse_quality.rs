//! Muse quality registration and post-edit enforcement through the real CLI.

use std::fs;
use std::io::{self, Write as _};
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
    let mut input = child
      .stdin
      .take()
      .ok_or_else(|| io::Error::other("piped stdin missing"))?;
    input.write_all(payload.as_bytes())?;
    drop(input);
    child.wait_with_output()
  }

  fn register_rule(&self, plugin: &str) -> io::Result<()> {
    let root = plugin_root(plugin).display().to_string();
    let args = [
      plugin,
      "hook",
      "register",
      "--event",
      "SessionStart",
      "--plugin-root",
      &root,
    ];
    let registered = self.run(&args, "{}")?;
    ensure(
      registered.status.code() == Some(0),
      format!("{plugin}: {registered:?}"),
    )?;
    let manifest = self
      .config
      .join(format!("toolu/post-tools.d/{plugin}@toolu__{plugin}.json"));
    ensure(manifest.is_file(), format!("missing manifest for {plugin}"))?;
    let bytes = fs::read(&manifest)?;
    let modified = fs::metadata(&manifest)?.modified()?;
    let again = self.run(&args, "{}")?;
    ensure(again.status.code() == Some(0), format!("{again:?}"))?;
    ensure(fs::read(&manifest)? == bytes, "manifest bytes changed")?;
    ensure(
      fs::metadata(&manifest)?.modified()? == modified,
      "manifest timestamp changed",
    )?;
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
      !pre.join("rust-quality@toolu__old.sh").exists(),
      "old shell bridge remains",
    )?;
    let lines = fs::read_to_string(&self.report)?;
    ensure(lines.lines().count() == 6, "startup report count differs")?;
    ensure(
      lines.contains(&post.display().to_string()),
      "startup report has no Muse manifest path",
    )?;
    Ok(())
  }

  fn assert_rust_edit_blocks_commit(&self) -> io::Result<()> {
    let bad = ["//! demo\n#[", "allow(dead_code)]\nfn bad() {}\n"].concat();
    let file = self.project.join("src/lib.rs");
    fs::write(&file, &bad)?;
    let payload = json!({"session_id":"s","cwd":self.project,"tool_name":"write_file",
      "tool_input":{"file_path":file,"content":bad},"tool_response":{}});
    let edited = self.run(
      &["hook", "post-tools", "--event", "PostToolUse"],
      &payload.to_string(),
    )?;
    ensure(edited.status.code() == Some(0), format!("{edited:?}"))?;
    let output: Value = serde_json::from_slice(&edited.stdout).map_err(io::Error::other)?;
    ensure(
      output
        .get("systemMessage")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("Forbidden lint suppression")),
      format!("missing quality notice: {output}"),
    )?;
    let gate = fs::read_to_string(self.project.join(".agents/tmp/quality-gate-status.json"))?;
    ensure(gate.contains("\"status\": \"failing\""), gate)?;
    let commit = json!({"session_id":"s","cwd":self.project,"tool_name":"bash",
      "tool_input":{"command":"git commit -m test"}});
    let before = self.run(
      &["hook", "pre-tools", "--event", "PreToolUse"],
      &commit.to_string(),
    )?;
    ensure(before.status.code() == Some(0), format!("{before:?}"))?;
    let decision: Value = serde_json::from_slice(&before.stdout).map_err(io::Error::other)?;
    ensure(
      decision.get("decision").and_then(Value::as_str) == Some("block"),
      format!("commit was not blocked: {decision}"),
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
fn muse_registers_three_rules_and_enforces_a_rust_edit() {
  let fixture = Fixture::new().unwrap();
  for plugin in ["ts-quality", "python-quality", "rust-quality"] {
    fixture.register_rule(plugin).unwrap();
  }
  fixture.assert_cleanup_and_report().unwrap();
  fixture.assert_rust_edit_blocks_commit().unwrap();
}
