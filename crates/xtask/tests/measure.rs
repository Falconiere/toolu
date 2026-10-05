//! `cargo xtask measure` through the real binary: one measured child per
//! process, so `RUSAGE_CHILDREN` holds exactly that child's tree.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const MIB: u64 = 1024 * 1024;

fn xtask(args: &[&str]) -> std::io::Result<Output> {
  Command::new(env!("CARGO_BIN_EXE_xtask"))
    .args(args)
    .output()
}

/// Measure `command` and return the report.
fn measured(command: &[&str]) -> Result<Value, String> {
  let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
  let out = dir.path().join("report.json");
  let out_arg = out.to_str().ok_or("temp path is not UTF-8")?;
  let mut args = vec!["measure", "--out", out_arg, "--"];
  args.extend_from_slice(command);
  let run = xtask(&args).map_err(|err| err.to_string())?;
  if !run.status.success() {
    return Err(String::from_utf8_lossy(&run.stderr).into_owned());
  }
  let text = std::fs::read_to_string(&out).map_err(|err| err.to_string())?;
  serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn number(report: &Value, key: &str) -> Option<u64> {
  report[key].as_u64()
}

#[test]
fn a_failing_command_is_measured_and_its_code_reported() {
  let report = measured(&["sh", "-c", "exit 3"]).unwrap();
  assert_eq!(report["version"], 1);
  assert_eq!(report["exitCode"], 3);
  assert_eq!(report["signal"], Value::Null);
  assert_eq!(report["command"], serde_json::json!(["sh", "-c", "exit 3"]));
  assert!(number(&report, "wallUs").unwrap() > 0);
}

#[test]
fn a_32_mib_string_shows_in_max_rss_and_true_stays_small() {
  let big = measured(&[
    "sh",
    "-c",
    "x=$(head -c 33554432 /dev/zero | tr '\\0' a); [ ${#x} -eq 33554432 ]",
  ])
  .unwrap();
  assert_eq!(big["exitCode"], 0);
  assert!(number(&big, "maxRssBytes").unwrap() >= 32 * MIB, "{big}");
  let tiny = measured(&["true"]).unwrap();
  assert!(number(&tiny, "maxRssBytes").unwrap() > 0, "{tiny}");
  assert!(number(&tiny, "maxRssBytes").unwrap() < 8 * MIB, "{tiny}");
}

#[test]
fn a_grandchild_s_cpu_is_counted() {
  let report = measured(&[
    "sh",
    "-c",
    "sh -c 'i=0; while [ $i -lt 300000 ]; do i=$((i+1)); done'",
  ])
  .unwrap();
  assert_eq!(report["exitCode"], 0);
  assert!(
    number(&report, "userUs").unwrap() + number(&report, "sysUs").unwrap() >= 50_000,
    "{report}"
  );
}

#[test]
fn stdin_reaches_the_command() {
  let dir = tempfile::tempdir().unwrap();
  let out = dir.path().join("report.json");
  let seen = dir.path().join("seen");
  let script = format!("cat > {}", seen.display());
  let mut child = Command::new(env!("CARGO_BIN_EXE_xtask"))
    .args([
      "measure",
      "--out",
      out.to_str().unwrap(),
      "--",
      "sh",
      "-c",
      &script,
    ])
    .stdin(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  std::io::Write::write_all(&mut child.stdin.take().unwrap(), b"{\"payload\":1}").unwrap();
  assert!(child.wait().unwrap().success());
  assert_eq!(std::fs::read_to_string(seen).unwrap(), "{\"payload\":1}");
}

#[test]
fn an_unspawnable_command_exits_2_and_names_it() {
  let dir = tempfile::tempdir().unwrap();
  let out = dir.path().join("report.json");
  let run = xtask(&[
    "measure",
    "--out",
    out.to_str().unwrap(),
    "--",
    "/no/such/command",
  ])
  .unwrap();
  assert_eq!(run.status.code(), Some(2));
  let stderr = String::from_utf8_lossy(&run.stderr);
  assert!(stderr.contains("cannot run /no/such/command"), "{stderr}");
  assert!(!Path::new(&out).exists());
}

#[test]
fn usage_errors_exit_2() {
  let no_out = xtask(&["measure", "--", "true"]).unwrap();
  assert_eq!(no_out.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&no_out.stderr).contains("measure needs --out FILE"));
  let no_command = xtask(&["measure", "--out", "/tmp/unused-report.json"]).unwrap();
  assert_eq!(no_command.status.code(), Some(2));
  let unwritable = xtask(&["measure", "--out", "/no/such/dir/report.json", "--", "true"]).unwrap();
  assert_eq!(unwritable.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&unwritable.stderr).contains("cannot write /no/such/dir"));
  let elsewhere = xtask(&["guardrails", "--", "true"]).unwrap();
  assert_eq!(elsewhere.status.code(), Some(2));
  assert!(String::from_utf8_lossy(&elsewhere.stderr).contains("only measure takes"));
}
