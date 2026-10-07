//! A not-yet-ported quality module through the Bun bridge (AC-6): the committed
//! ts-quality bundle, registered as `ts-quality@toolu__ts-quality.js` beside a
//! `.sh` module, checks a TypeScript edit with the same output and the same
//! gate entry whether the TypeScript dispatcher (run live through
//! `dispatch-cli.ts`) or the engine runs it, each in its own identical sandbox.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::io::Write as _;
use std::process::{Command, Stdio};

use hook::Hook;
use sandbox::{Res, write};
use serde_json::{Value, json};
use toolu_engine::{ModuleResult, Phase};
use toolu_protocol::host::Host;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");

/// `n` code lines of TypeScript.
fn source(n: usize) -> String {
  "export const v = 0;\n".repeat(n)
}

fn git(hook: &Hook, args: &[&str]) -> Res<()> {
  let ok = Command::new("git")
    .args([
      "-c",
      "user.email=t@t",
      "-c",
      "user.name=t",
      "-c",
      "commit.gpgsign=false",
    ])
    .args(args)
    .current_dir(hook.sb.path("project"))
    .status()
    .map_err(|err| err.to_string())?
    .success();
  if ok {
    Ok(())
  } else {
    Err(format!("git {args:?} failed"))
  }
}

/// A tracked TypeScript project with a 10-line limit, the ts-quality bundle and a `.sh` note.
fn project(edited: &str) -> Res<Hook> {
  let hook = Hook::new(Host::Claude)?;
  write(&hook.sb.path("project/tsconfig.json"), "{}\n")?;
  write(&hook.sb.path("project/package.json"), "{\"name\":\"x\"}\n")?;
  write(&hook.sb.path("project/bun.lock"), "")?;
  write(
    &hook.sb.path("project/.claude/toolu.config.json"),
    r#"{"version":1,"lang":{"ts":{"maxFileLines":10}}}"#,
  )?;
  git(&hook, &["add", "-A"])?;
  git(&hook, &["commit", "-q", "-m", "project"])?;
  write(&hook.sb.path("project/src/a.ts"), edited)?;
  let bundle = std::fs::read_to_string(format!(
    "{REPO}/plugins/ts-quality/hooks/dist/post-tool-use.js"
  ))
  .map_err(|err| err.to_string())?;
  write(
    &hook
      .dir(Phase::Post)
      .join("ts-quality@toolu__ts-quality.js"),
    &bundle,
  )?;
  hook.sh(
    Phase::Post,
    "zz@t__note.sh",
    r#"printf '%s\n' '{"systemMessage":"note"}'"#,
  )?;
  Ok(hook)
}

fn payload(hook: &Hook) -> String {
  let file = hook.sb.text("project/src/a.ts");
  json!({
    "hook_event_name": "PostToolUse", "session_id": "s1", "cwd": hook.sb.text("project"),
    "tool_name": "Edit",
    "tool_input": {"file_path": file, "old_string": "a", "new_string": "b"},
    "tool_response": {"filePath": file, "success": true},
  })
  .to_string()
}

/// The TypeScript dispatcher's answer for `hook`, run as a process.
fn typescript(hook: &Hook) -> Res<ModuleResult> {
  let env: serde_json::Map<String, Value> = hook
    .env()
    .vars()
    .map(|(key, value)| (key.to_owned(), Value::from(value)))
    .collect();
  let request = json!({
    "phase": "post", "stdin": payload(hook), "env": env,
    "cwd": hook.sb.text("project"), "libDir": hook.sb.text("plugin/hooks/lib"),
  });
  let mut child = Command::new("bun")
    .arg(format!(
      "{REPO}/packages/toolu-core/src/dispatch/__tests__/dispatch-cli.ts"
    ))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| err.to_string())?;
  let mut stdin = child.stdin.take().ok_or("no stdin")?;
  stdin
    .write_all(request.to_string().as_bytes())
    .map_err(|err| err.to_string())?;
  drop(stdin);
  let out = child.wait_with_output().map_err(|err| err.to_string())?;
  let result: Value = serde_json::from_slice(&out.stdout)
    .map_err(|err| format!("{err}: {}", String::from_utf8_lossy(&out.stderr)))?;
  let field = |key: &str| {
    result
      .get(key)
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_owned()
  };
  let code = result.get("exitCode").and_then(Value::as_i64).unwrap_or(-1);
  Ok(ModuleResult {
    stdout: field("stdout"),
    stderr: field("stderr"),
    exit_code: i32::try_from(code).unwrap_or(-1),
  })
}

/// The output and the quality gate file, with the sandbox root and timestamps neutralized.
fn observed(hook: &Hook, result: &ModuleResult) -> (String, String, i32, String) {
  let root = hook.sb.root.to_string_lossy().into_owned();
  let gate = std::fs::read_to_string(hook.sb.path("project/.claude/tmp/quality-gate-status.json"))
    .unwrap_or_default();
  let neutral = |text: &str| {
    let text = text.replace(&root, "<ROOT>");
    regex_free_timestamps(&text)
  };
  (
    neutral(&result.stdout),
    neutral(&result.stderr),
    result.exit_code,
    neutral(&gate),
  )
}

/// Every `"…At": "<value>"` value (the gate file is pretty-printed) as `"<time>"`.
fn regex_free_timestamps(text: &str) -> String {
  const KEY: &str = "At\": \"";
  let mut out = String::new();
  let mut rest = text;
  while let Some(at) = rest.find(KEY) {
    let (head, tail) = rest.split_at(at + KEY.len());
    out.push_str(head);
    let end = tail.find('"').unwrap_or(tail.len());
    out.push_str("<time>");
    rest = tail.get(end..).unwrap_or_default();
  }
  out.push_str(rest);
  out
}

#[test]
fn the_ts_quality_bundle_judges_an_edit_the_same_through_the_bridge() {
  for (edited, violates) in [(source(12), true), (source(3), false)] {
    let ts = project(&edited).unwrap();
    let rust = project(&edited).unwrap();
    let ts_result = typescript(&ts).unwrap();
    let rust_result = rust.run(Phase::Post, &payload(&rust), &[], &[]).result;
    let (ts_seen, rust_seen) = (observed(&ts, &ts_result), observed(&rust, &rust_result));
    assert_eq!(rust_seen, ts_seen);
    assert_eq!(
      rust_seen.0.contains("exceeds 10-line limit"),
      violates,
      "{}",
      rust_seen.0
    );
    assert!(
      rust_seen.0.contains("note"),
      "the .sh module ran too: {}",
      rust_seen.0
    );
    let stamped = rust_seen.3.contains("\"updatedAt\": \"<time>\"");
    assert_eq!(
      stamped, violates,
      "a violation records a stamped entry: {}",
      rust_seen.3
    );
  }
}
