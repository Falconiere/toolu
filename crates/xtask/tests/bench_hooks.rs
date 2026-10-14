//! `cargo xtask bench-hooks` end to end through the real binary: a temp plugin
//! tree whose `hooks.json` holds generated native launchers, a stand-in `toolu`
//! that answers the launcher's probe and exits 0, and the real measurer.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use toolu_protocol::launcher::{DEFAULT_TIMEOUT, Target, hook};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn write(path: &Path, text: &str) -> Res<()> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(path, text)?;
  Ok(())
}

/// A repository root with one plugin, `demo`, and its `pre-tools` and `session-start` hooks.
fn tree(root: &Path, budget_cpu_ms: u64) -> Res<()> {
  let mut events = serde_json::Map::new();
  for (event, name) in [
    ("PreToolUse", "pre-tools"),
    ("SessionStart", "session-start"),
  ] {
    let generated = hook(
      &Target {
        plugin: "demo",
        event,
        name,
      },
      DEFAULT_TIMEOUT,
    )?;
    let entry = json!({"type": "command", "command": generated.command,
      "commandWindows": generated.command_windows, "timeout": generated.timeout});
    events.insert(event.to_owned(), json!([{"hooks": [entry]}]));
  }
  write(
    &root.join("plugins/demo/hooks/hooks.json"),
    &json!({"hooks": events}).to_string(),
  )?;
  let payloads = json!({"version": 1, "entries": {
    "demo/pre-tools": {"stdin": {"cwd": "${PROJECT}", "tool_name": "Bash"}},
    "demo/session-start": {"stdin": {"source": "startup"}}}});
  write(
    &root.join("benchmarks/cases/hooks/payloads.json"),
    &payloads.to_string(),
  )?;
  let budgets = json!({"version": 1, "percentile": 50,
    "entries": {"demo/pre-tools": {"maxRssMiB": 64, "cpuMs": budget_cpu_ms}}});
  write(
    &root.join("benchmarks/hook-budgets.json"),
    &budgets.to_string(),
  )?;
  let toolu = root.join("bin/toolu");
  write(
    &toolu,
    "#!/bin/sh\n[ \"$1\" = --hook-protocol ] && echo 2\ncat >/dev/null\nexit 0\n",
  )?;
  std::fs::set_permissions(&toolu, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

fn bench(root: &Path, extra: &[&str]) -> Res<Output> {
  let out = root.join("result.json");
  Ok(
    Command::new(env!("CARGO_BIN_EXE_xtask"))
      .args(["bench-hooks", "--root"])
      .arg(root)
      .arg("--bin")
      .arg(root.join("bin/toolu"))
      .args(["--runs", "2", "--out"])
      .arg(out)
      .args(extra)
      .output()?,
  )
}

#[test]
fn every_entry_is_measured_written_and_judged() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  tree(&root, 1000).unwrap();
  let run = bench(&root, &["--assert"]).unwrap();
  let stdout = String::from_utf8_lossy(&run.stdout);
  assert_eq!(run.status.code(), Some(0), "{run:?}");
  assert!(
    stdout.contains("| demo/pre-tools | PreToolUse | rust |"),
    "{stdout}"
  );
  assert!(
    stdout.contains("| demo/session-start | SessionStart | rust |"),
    "{stdout}"
  );
  let doc: Value =
    serde_json::from_str(&std::fs::read_to_string(root.join("result.json")).unwrap()).unwrap();
  assert_eq!(doc["schema"], "toolu.hook-resources/v1");
  assert_eq!(doc["provenance"]["runs"], 2);
  assert_eq!(doc["entries"].as_array().unwrap().len(), 2);
  assert!(doc["entries"][0]["maxRssBytes"]["p50"].as_u64().unwrap() > 0);
}

#[test]
fn an_entry_over_its_budget_fails_and_names_it() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  tree(&root, 0).unwrap();
  let run = bench(&root, &["--assert"]).unwrap();
  let stderr = String::from_utf8_lossy(&run.stderr);
  assert_eq!(run.status.code(), Some(1), "{run:?}");
  assert!(stderr.contains("demo/pre-tools: cpu p50"), "{stderr}");
  let only = bench(&root, &["--only", "demo/session-start", "--assert"]).unwrap();
  assert_eq!(only.status.code(), Some(0), "{only:?}");
}

#[test]
fn a_budget_for_an_entry_no_hook_registers_fails_even_when_only_filters_it_out() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  tree(&root, 1000).unwrap();
  let budgets = json!({"version": 1, "percentile": 50, "entries": {
    "demo/pre-tools": {"maxRssMiB": 64, "cpuMs": 1000},
    "demo/renamed-hook": {"maxRssMiB": 64, "cpuMs": 1000}}});
  write(
    &root.join("benchmarks/hook-budgets.json"),
    &budgets.to_string(),
  )
  .unwrap();
  for extra in [
    vec!["--assert"],
    vec!["--only", "demo/session-start", "--assert"],
  ] {
    let run = bench(&root, &extra).unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert_eq!(run.status.code(), Some(1), "{run:?}");
    assert!(
      stderr.contains("demo/renamed-hook: a budget for an entry no hooks.json registers"),
      "{stderr}"
    );
  }
}
