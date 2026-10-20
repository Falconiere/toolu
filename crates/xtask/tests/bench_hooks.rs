//! `cargo xtask bench-hooks` end to end through the real binary: a temp plugin
//! tree whose `hooks.json` holds generated native launchers, a stand-in `toolu`
//! that answers the launcher's probe and exits 0, and the real measurer.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use toolu_protocol::launcher::muse::{DEFAULT_TIMEOUT_MS, hook as muse_hook};
use toolu_protocol::launcher::{DEFAULT_TIMEOUT, Target, hook};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn write(path: &Path, text: &str) -> Res<()> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(path, text)?;
  Ok(())
}

/// A repository root with one plugin, `demo`, and its `pre-tools` and
/// `session-start` hooks, in `hooks.json` and in its generated Muse manifest.
fn tree(root: &Path, budget_cpu_ms: u64) -> Res<()> {
  let mut events = serde_json::Map::new();
  let mut muse = Vec::new();
  for (event, name) in [
    ("PreToolUse", "pre-tools"),
    ("SessionStart", "session-start"),
  ] {
    let target = Target {
      plugin: "demo",
      event,
      name,
    };
    let generated = hook(&target, DEFAULT_TIMEOUT)?;
    let entry = json!({"type": "command", "command": generated.command,
      "commandWindows": generated.command_windows, "timeout": generated.timeout});
    events.insert(event.to_owned(), json!([{"hooks": [entry]}]));
    let resolver = muse_hook(&target, DEFAULT_TIMEOUT_MS)?;
    write(
      &root.join("plugins/demo").join(&resolver.resolver_path),
      &resolver.resolver,
    )?;
    muse.push(serde_json::to_value(&resolver.entry)?);
  }
  write(
    &root.join("plugins/demo/hooks/hooks.json"),
    &json!({"hooks": events}).to_string(),
  )?;
  write(
    &root.join("plugins/demo/.muse-plugin/plugin.json"),
    &json!({"schemaVersion": 1, "capabilities": {"hooks": muse}}).to_string(),
  )?;
  let payloads = json!({"version": 1, "entries": {
    "demo/pre-tools": {"stdin": {"cwd": "${PROJECT}", "tool_name": "Bash"}},
    "demo/session-start": {"stdin": {"source": "startup"}},
    "muse:demo/pre-tools": {"stdin": {"cwd": "${PROJECT}", "tool_name": "bash"}},
    "muse:demo/session-start": {"stdin": {"source": "startup"}}}});
  write(
    &root.join("benchmarks/cases/hooks/payloads.json"),
    &payloads.to_string(),
  )?;
  let budget = json!({"maxRssMiB": 64, "cpuMs": budget_cpu_ms});
  let budgets = json!({"version": 1, "percentile": 50,
    "entries": {"demo/pre-tools": budget, "muse:demo/pre-tools": budget}});
  write(
    &root.join("benchmarks/hook-budgets.json"),
    &budgets.to_string(),
  )?;
  let toolu = root.join("bin/toolu");
  write(
    &toolu,
    "#!/bin/sh\n[ \"$1\" = --hook-protocol ] && echo 2 && exit 0\n\
     [ -n \"$MUSE_PLUGIN_ID\" ] && echo \"$*\" >> \"$0.muse.log\"\ncat >/dev/null\nexit 0\n",
  )?;
  std::fs::set_permissions(&toolu, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

fn bench(root: &Path, extra: &[&str]) -> Res<Output> {
  let out = root.join("result.json");
  Ok(
    Command::new(env!("CARGO_BIN_EXE_xtask"))
      // The test binary measures itself, so no release `xtask` is built.
      .env("XTASK_MEASURER", env!("CARGO_BIN_EXE_xtask"))
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
  assert!(
    stdout.contains("| muse:demo/pre-tools | PreToolUse | rust |"),
    "{stdout}"
  );
  let doc: Value =
    serde_json::from_str(&std::fs::read_to_string(root.join("result.json")).unwrap()).unwrap();
  assert_eq!(doc["schema"], "toolu.hook-resources/v1");
  assert_eq!(doc["provenance"]["runs"], 2);
  let entries = doc["entries"].as_array().unwrap();
  assert_eq!(entries.len(), 4);
  assert_eq!(entries[2]["entry"], "muse:demo/pre-tools");
  assert!(entries[2]["maxRssBytes"]["p50"].as_u64().unwrap() > 0);
  assert!(entries[2]["cpuUs"]["p90"].as_u64().is_some(), "{}", entries[2]);
  // The resolver found the stand-in through TOOLU_BIN and ran the hook with
  // Muse's plugin root, once per warm-up and measured run of each Muse entry.
  let calls = std::fs::read_to_string(root.join("bin/toolu.muse.log")).unwrap();
  let root_text = root.join("plugins/demo").display().to_string();
  let pre = format!("demo hook pre-tools --event PreToolUse --plugin-root {root_text}");
  assert_eq!(calls.lines().filter(|line| *line == pre).count(), 4, "{calls}");
  let demo = calls.lines().filter(|line| line.starts_with("demo hook "));
  assert_eq!(demo.count(), 8, "{calls}");
  // The sandbox also registered the four registry plugins under Muse's environment.
  let registered = calls.lines().filter(|line| line.contains(" hook register "));
  assert_eq!(registered.count(), 4, "{calls}");
}

#[test]
fn an_entry_over_its_budget_fails_and_names_it() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  tree(&root, 0).unwrap();
  let run = bench(&root, &["--assert"]).unwrap();
  let stderr = String::from_utf8_lossy(&run.stderr);
  assert_eq!(run.status.code(), Some(1), "{run:?}");
  let claude = |line: &str| line.contains("demo/pre-tools: cpu p50") && !line.contains("muse:");
  assert!(stderr.lines().any(claude), "{stderr}");
  assert!(stderr.contains("muse:demo/pre-tools: cpu p50"), "{stderr}");
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
    "demo/renamed-hook": {"maxRssMiB": 64, "cpuMs": 1000},
    "muse:demo/renamed-hook": {"maxRssMiB": 64, "cpuMs": 1000}}});
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
      stderr.contains("demo/renamed-hook: a budget for an entry no hooks.json or Muse manifest registers"),
      "{stderr}"
    );
    assert!(stderr.contains("muse:demo/renamed-hook: a budget"), "{stderr}");
  }
}
