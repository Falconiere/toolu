//! Native debug summaries against captured Bun, Cargo, Node and Rust output.

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn fixture(name: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins/toolu/scripts/__tests__/fixtures/debug")
    .join(name)
}

fn same<T: PartialEq + Debug>(actual: &T, expected: &T) -> Res<()> {
  if actual == expected {
    Ok(())
  } else {
    Err(std::io::Error::other(format!("got {actual:?}, expected {expected:?}")).into())
  }
}

fn require(condition: bool, message: &str) -> Res<()> {
  if condition {
    Ok(())
  } else {
    Err(std::io::Error::other(message).into())
  }
}

fn run(verb: &str, file: Option<&Path>, json: bool, stdin: Option<&str>) -> Res<Output> {
  let mut command = assert_cmd::Command::new(TOOLU);
  if json {
    command.arg("--json");
  }
  command.args(["debug", verb]);
  if let Some(path) = file {
    command.arg("--file").arg(path);
  }
  if let Some(input) = stdin {
    command.write_stdin(input);
  }
  Ok(command.output()?)
}

fn json(output: &Output) -> Res<Value> {
  require(
    output.status.success(),
    &String::from_utf8_lossy(&output.stderr),
  )?;
  Ok(serde_json::from_slice(&output.stdout)?)
}

fn run_cap(verb: &str, file: &Path, cap: &str, value: &str) -> Res<Output> {
  Ok(
    assert_cmd::Command::new(TOOLU)
      .args(["debug", verb, "--file"])
      .arg(file)
      .env(cap, value)
      .output()?,
  )
}

#[test]
fn io_counts_real_log_bytes_and_lines() -> Res<()> {
  let path = fixture("big.log");
  let bytes = fs::read(&path)?;
  let lines =
    bytes.split(|byte| *byte == b'\n').count() - usize::from(bytes.last() == Some(&b'\n'));
  let result = json(&run("io", Some(&path), true, None)?)?;
  same(&result["bytes"].as_u64(), &Some(bytes.len() as u64))?;
  same(&result["lines"].as_u64(), &Some(lines as u64))?;
  let empty = json(&run("io", None, true, Some(""))?)?;
  same(&empty, &serde_json::json!({"bytes": 0, "lines": 0}))
}

#[test]
fn log_summary_is_bounded_and_reports_the_captured_total() -> Res<()> {
  let path = fixture("big.log");
  let parsed = json(&run("log", Some(&path), true, None)?)?;
  require(
    parsed["total_lines"]
      .as_u64()
      .is_some_and(|count| count > 1000),
    "missing or invalid log total",
  )?;
  require(
    parsed["errors"]
      .as_array()
      .is_some_and(|rows| !rows.is_empty()),
    "no errors",
  )?;
  same(&parsed["truncated"], &Value::Bool(true))?;
  let text = run("log", Some(&path), false, None)?;
  require(text.status.success(), "log text failed")?;
  require(text.stdout.len() <= 65_536, "log text exceeded byte cap")?;
  require(
    String::from_utf8_lossy(&text.stdout).contains("TOTAL lines:"),
    "no total line",
  )
}

#[test]
fn stack_summary_finds_app_frames_in_both_real_traces() -> Res<()> {
  for (file, needle) in [
    ("stacktrace.txt", "inner"),
    ("rust-panic.txt", "level_three"),
  ] {
    let parsed = json(&run("stack", Some(&fixture(file)), true, None)?)?;
    same(&parsed["recognized"], &Value::Bool(true))?;
    require(
      parsed["noise_frames"].as_u64().unwrap_or(0) > 0,
      "no collapsed frames",
    )?;
    require(
      parsed["app_frames"].to_string().contains(needle),
      "missing app frame",
    )?;
  }
  let raw = run("stack", None, false, Some("not a stack\n"))?;
  require(
    String::from_utf8_lossy(&raw.stdout).contains("no recognizable stack frames"),
    "no raw fallback",
  )
}

#[test]
fn testfail_summary_reads_bun_and_cargo_transcripts() -> Res<()> {
  for (file, name) in [
    ("bun-testfail.txt", "add sums two numbers"),
    ("cargo-testfail.txt", "tests::adds"),
  ] {
    let parsed = json(&run("testfail", Some(&fixture(file)), true, None)?)?;
    same(&parsed["recognized"], &Value::Bool(true))?;
    require(
      parsed["failures"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row.as_str() == Some(name))),
      "missing failed test",
    )?;
    require(
      parsed["locations"]
        .as_array()
        .is_some_and(|rows| rows.first().is_some_and(Value::is_string)),
      "missing source location",
    )?;
  }
  Ok(())
}

#[test]
fn unreadable_file_and_bad_arguments_fail_visibly() -> Res<()> {
  for verb in ["io", "log", "stack", "testfail"] {
    let missing = run(verb, Some(&fixture("missing.txt")), false, None)?;
    require(!missing.status.success(), "missing file succeeded")?;
    require(
      String::from_utf8_lossy(&missing.stderr).contains("cannot read file"),
      "missing file silent",
    )?;
    let bad = assert_cmd::Command::new(TOOLU)
      .args(["debug", verb, "--file"])
      .output()?;
    require(!bad.status.success(), "missing --file value succeeded")?;
  }
  Ok(())
}

#[test]
fn low_caps_and_unrecognized_text_stay_bounded() -> Res<()> {
  let log = run_cap("log", &fixture("big.log"), "DEBUG_MAX_LINES", "10")?;
  require(
    String::from_utf8_lossy(&log.stdout).lines().count() <= 10,
    "log line cap",
  )?;
  let bytes = run_cap("log", &fixture("big.log"), "DEBUG_MAX_BYTES", "200")?;
  require(bytes.stdout.len() <= 200, "log byte cap")?;
  require(
    String::from_utf8_lossy(&bytes.stdout).contains("output truncated"),
    "no byte marker",
  )?;
  let stack = run_cap("stack", &fixture("rust-panic.txt"), "DEBUG_MAX_FRAMES", "1")?;
  require(
    String::from_utf8_lossy(&stack.stdout).contains("more"),
    "stack frame cap",
  )?;
  let tests = run_cap(
    "testfail",
    &fixture("bun-testfail.txt"),
    "DEBUG_MAX_FAILURES",
    "1",
  )?;
  require(
    String::from_utf8_lossy(&tests.stdout)
      .starts_with("FAILED TESTS (1+):\n  - add sums two numbers\n  ... (+1 more)"),
    "test cap",
  )?;
  let unknown = run("testfail", None, false, Some("hello\nworld\n"))?;
  require(
    String::from_utf8_lossy(&unknown.stdout).contains("no recognizable test failures"),
    "test raw fallback",
  )
}
