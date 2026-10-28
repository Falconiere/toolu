//! The git-better plugin end to end: `SessionStart` registration publishes the
//! `semantic-git` manifest, then toolu's own `hook pre-tools` replays every case
//! of `fixtures/git-better/commands.json` on claude, codex and muse in a real
//! repository.

#[path = "helpers/feed.rs"]
mod feed;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
const HOSTS: [&str; 3] = ["claude", "codex", "muse"];
const CASES: usize = 16;

fn git(root: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-C"])
    .arg(root)
    .args(args)
    .stdout(Stdio::null())
    .status()?;
  if !status.success() {
    return Err(format!("git {args:?} failed").into());
  }
  Ok(())
}

/// Where `host` reads a project's `toolu.config.json`.
fn config_dir(host: &str) -> &'static str {
  match host {
    "codex" => ".codex",
    "muse" => ".agents",
    _ => ".claude",
  }
}

/// A repository on `main` with branches `feat/1-x` and `legacy`, git-better
/// registered in its own config root.
fn project(dir: &Path, case: &Value, host: &str) -> Res<(PathBuf, PathBuf)> {
  let project = dir.join("project");
  let config = dir.join("config");
  let settings = project.join(config_dir(host));
  fs::create_dir_all(&settings)?;
  git(&project, &["init", "-q", "-b", "main"])?;
  git(&project, &["commit", "-q", "--allow-empty", "-m", "init"])?;
  git(&project, &["branch", "feat/1-x"])?;
  git(&project, &["branch", "legacy"])?;
  if let Some(branch) = case["branch"].as_str() {
    git(&project, &["switch", "-q", branch])?;
  }
  if let Some(text) = case["config"].as_str() {
    fs::write(settings.join("toolu.config.json"), text)?;
  }
  let output = Command::new(TOOLU)
    .args(["git-better", "hook", "register", "--event", "SessionStart"])
    .env("TOOLU_CONFIG_DIR", &config)
    .env("HOME", dir)
    .current_dir(&project)
    .output()?;
  let stdout = String::from_utf8(output.stdout)?;
  if !output.status.success() || !stdout.is_empty() {
    return Err(format!("registration failed: {stdout}").into());
  }
  Ok((project, config))
}

fn pre_tools(dir: &Path, project: &Path, config: &Path, host: &str, command: &str) -> Res<Output> {
  let payload = json!({
    "session_id": "git-better",
    "hook_event_name": "PreToolUse",
    "tool_name": "Bash",
    "tool_input": { "command": command },
    "cwd": project,
  });
  let mut child = Command::new(TOOLU)
    .args(["hook", "pre-tools", "--event", "PreToolUse"])
    .env("TOOLU_CONFIG_DIR", config)
    .env("TOOLU_HOST_OVERRIDE", host)
    .env("TOOLU_SETTINGS_DIR", dir.join("settings"))
    .env("CLAUDE_PROJECT_DIR", project)
    .env("HOME", dir)
    .current_dir(project)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  feed::feed(child.stdin.take(), payload.to_string().as_bytes())?;
  Ok(child.wait_with_output()?)
}

/// `("deny", reason)`, `("advise", text)` or `("allow", stdout)`, per host encoding.
fn verdict(output: &Output) -> Res<(&'static str, String)> {
  if output.stdout.is_empty() {
    return Ok(("allow", String::new()));
  }
  let doc: Value = serde_json::from_slice(&output.stdout)?;
  let text = |pointer: &str| {
    doc
      .pointer(pointer)
      .and_then(Value::as_str)
      .map(str::to_owned)
  };
  if doc.pointer("/hookSpecificOutput/permissionDecision") == Some(&json!("deny")) {
    return Ok((
      "deny",
      text("/hookSpecificOutput/permissionDecisionReason").unwrap_or_default(),
    ));
  }
  if doc.get("decision") == Some(&json!("block")) {
    return Ok(("deny", text("/reason").unwrap_or_default()));
  }
  let advisory = text("/hookSpecificOutput/additionalContext").or_else(|| text("/systemMessage"));
  Ok(match advisory {
    Some(message) if message.contains("git-better") => ("advise", message),
    Some(message) => ("allow", message),
    None => ("allow", String::new()),
  })
}

fn replay(case: &Value, host: &str) -> Res<()> {
  let name = case["name"].as_str().ok_or("case name")?;
  let command = case["command"].as_str().ok_or("case command")?;
  let expect = case["expect"].as_str().ok_or("case expect")?;
  let dir = tempfile::tempdir()?;
  let (project, config) = project(dir.path(), case, host)?;
  let output = pre_tools(dir.path(), &project, &config, host, command)?;
  if !output.status.success() {
    return Err(format!("{name} [{host}]: {output:?}").into());
  }
  let (found, text) = verdict(&output)?;
  if found != expect {
    return Err(format!("{name} [{host}]: expected {expect}, got {found}: {text}").into());
  }
  if expect == "allow" && text.contains("git-better") {
    return Err(format!("{name} [{host}]: allowed with git-better text: {text}").into());
  }
  if let Some(contains) = case["contains"].as_str()
    && !text.contains(contains)
  {
    return Err(format!("{name} [{host}]: missing {contains:?} in {text}").into());
  }
  Ok(())
}

#[test]
fn every_fixture_case_is_decided_the_same_on_every_host() -> Res<()> {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/git-better/commands.json");
  let fixture: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
  let cases = fixture["cases"].as_array().ok_or("cases")?;
  if cases.len() != CASES {
    return Err(format!("expected {CASES} cases, found {}", cases.len()).into());
  }
  for case in cases {
    for host in HOSTS {
      replay(case, host)?;
    }
  }
  Ok(())
}
