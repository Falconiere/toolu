//! Real `SessionStart` manifest registration and native pre/post hook dispatch.

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn plugin_root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/ast-grep")
}

fn project(dir: &Path) -> Res<PathBuf> {
  let project = dir.join("project");
  fs::create_dir_all(&project)?;
  let git = Command::new("git")
    .args(["init", "--quiet"])
    .current_dir(&project)
    .status()?;
  if !git.success() {
    return Err("git init failed".into());
  }
  Ok(project)
}

fn register(config: &Path, project: &Path) -> Res<()> {
  for _ in 0..2 {
    let status = Command::new(TOOLU)
      .args([
        "ast-grep",
        "hook",
        "register",
        "--event",
        "SessionStart",
        "--plugin-root",
      ])
      .arg(plugin_root())
      .env("TOOLU_CONFIG_DIR", config)
      .env("HOME", project)
      .current_dir(project)
      .status()?;
    if !status.success() {
      return Err("SessionStart registration failed".into());
    }
  }
  Ok(())
}

struct HookCall<'a> {
  project: &'a Path,
  config: &'a Path,
  host: &'a str,
  phase: &'a str,
  payload: &'a Value,
  path: &'a str,
}

fn hook(call: &HookCall<'_>) -> Res<Output> {
  let mut process = Command::new(TOOLU);
  process
    .args([
      "hook",
      call.phase,
      "--event",
      if call.phase == "pre-tools" {
        "PreToolUse"
      } else {
        "PostToolUse"
      },
    ])
    .env("TOOLU_CONFIG_DIR", call.config)
    .env("TOOLU_HOST_OVERRIDE", call.host)
    .env("CLAUDE_PROJECT_DIR", call.project)
    .env("HOME", call.project)
    .env("PATH", call.path)
    .current_dir(call.project)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = process.spawn()?;
  child
    .stdin
    .take()
    .ok_or("no stdin")?
    .write_all(call.payload.to_string().as_bytes())?;
  Ok(child.wait_with_output()?)
}

fn message(output: &Output) -> Res<Option<String>> {
  if output.stdout.is_empty() {
    return Ok(None);
  }
  let doc: Value = serde_json::from_slice(&output.stdout)?;
  Ok(
    doc
      .pointer("/hookSpecificOutput/additionalContext")
      .or_else(|| doc.get("systemMessage"))
      .and_then(Value::as_str)
      .map(str::to_owned),
  )
}

fn nudge_case(
  case: &Value,
  golden: &Value,
  dir: &Path,
  project: &Path,
  index: usize,
) -> Res<(Option<String>, Option<String>)> {
  let config = dir.join(format!("config-{index}"));
  fs::create_dir_all(&config)?;
  register(&config, project)?;
  if case["state"] == "opt-out" {
    fs::write(
      config.join("toolu.config.json"),
      "{\"version\":1,\"skills\":{\"ast-grep\":false}}",
    )?;
  }
  let name = case["name"].as_str().ok_or("no case name")?;
  let payload = json!({"session_id":"nudge", "tool_name":case["toolName"],
    "tool_input":case["toolInput"]});
  let path = if case["state"] == "missing" {
    config.display().to_string()
  } else {
    std::env::var("PATH")?
  };
  let output = hook(&HookCall {
    project,
    config: &config,
    host: "claude",
    phase: "pre-tools",
    payload: &payload,
    path: &path,
  })?;
  if !output.status.success() {
    return Err(format!("{name}: {output:?}").into());
  }
  let actual = message(&output)?;
  let key = format!("{name} [claude]");
  let baseline = golden
    .get("nudge")
    .and_then(|cases| cases.get(&key))
    .and_then(|case| case.get("stdout"))
    .and_then(Value::as_str)
    .ok_or("golden case")?;
  let expected = if baseline.is_empty() {
    None
  } else {
    message(&Output {
      status: output.status,
      stdout: baseline.as_bytes().to_vec(),
      stderr: Vec::new(),
    })?
  };
  Ok((actual, expected))
}

#[test]
fn nudge_manifest_matches_every_real_fixture_case() {
  let dir = tempfile::tempdir().expect("sandbox");
  let project = project(dir.path()).expect("project");
  let fixture: Value = serde_json::from_str(
    &fs::read_to_string(
      Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ast-grep/nudge.json"),
    )
    .expect("nudge fixture"),
  )
  .expect("nudge JSON");
  let golden: Value = serde_json::from_str(
    &fs::read_to_string(
      Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ast-grep/golden.json"),
    )
    .expect("golden fixture"),
  )
  .expect("golden JSON");
  let cases = fixture["cases"].as_array().expect("nudge cases");
  for (index, case) in cases.iter().enumerate() {
    let name = case["name"].as_str().expect("case name");
    let (actual, expected) = nudge_case(case, &golden, dir.path(), &project, index)
      .unwrap_or_else(|error| panic!("{name}: {error}"));
    if case["deviation"]["silent"] == true {
      assert_eq!(actual, None, "{name}");
    } else if let Some(contains) = case["deviation"]["contains"].as_str() {
      assert!(
        actual
          .as_deref()
          .is_some_and(|text| text.contains(contains)),
        "{name}: {actual:?}"
      );
      if let Some(excludes) = case["deviation"]["excludes"].as_str() {
        assert!(
          !actual
            .as_deref()
            .is_some_and(|text| text.contains(excludes)),
          "{name}"
        );
      }
    } else {
      assert_eq!(actual, expected, "{name}");
    }
  }
}

fn post_output(
  project: &Path,
  config: &Path,
  host: &str,
  payload: &Value,
  path: &str,
) -> Res<Output> {
  hook(&HookCall {
    project,
    config,
    host,
    phase: "post-tools",
    payload,
    path,
  })
}

#[test]
fn post_manifest_records_real_result_and_reports_only_on_opencode() {
  let dir = tempfile::tempdir().expect("sandbox");
  let project = project(dir.path()).expect("project");
  let config = dir.path().join("config");
  register(&config, &project).expect("registration");
  let payload = json!({"session_id":"ses_opencode-1", "tool_name":"Bash",
    "tool_input":{"command":"ast-grep run -p 'console.log($A)' -l typescript src"},
    "tool_response":{"output":"match line\n"}});
  let path = std::env::var("PATH").expect("PATH");
  let claude = post_output(&project, &config, "claude", &payload, &path).expect("Claude post hook");
  assert!(claude.status.success(), "{claude:?}");
  assert_eq!(message(&claude).expect("Claude output"), None);
  let open =
    post_output(&project, &config, "opencode", &payload, &path).expect("OpenCode post hook");
  assert!(open.status.success(), "{open:?}");
  assert!(
    message(&open)
      .expect("OpenCode output")
      .is_some_and(|text| text.contains("ast-grep: returned=20 (n=2)"))
  );
  let ledger = config.join("toolu/byte-savings/sesopencode-1.jsonl");
  assert_eq!(
    fs::read_to_string(ledger).expect("ledger"),
    "{\"kind\":\"ast-grep\",\"returned\":10,\"full\":0}\n".repeat(2)
  );
}
