//! The step-check runner on real processes and the evidence encoding
//! (`ledger-check.test.ts`), with values `ledger-check.ts` computes.

use std::path::Path;
use std::time::{Duration, Instant};

use toolu_runtime::env::Env;

use super::{CheckRun, TIMEOUT_EXIT, evidence_of, parse_timeout, run_check, step_evidence};
use crate::resources::binding::bind_worktree;

fn run(dir: &Path, check: &str, timeout: &str) -> (i32, String) {
  let path = std::env::var("PATH").unwrap();
  let out_file = dir.join("out.txt");
  let spec = CheckRun {
    check: check.to_owned(),
    cwd: dir.to_path_buf(),
    env: Env::from_pairs([("PATH", path.as_str())]),
    out_file: out_file.clone(),
    timeout: timeout.to_owned(),
  };
  let code = run_check(&spec).unwrap();
  (code, std::fs::read_to_string(out_file).unwrap())
}

#[test]
fn output_interleaves_into_one_file_and_stdin_is_at_end_of_file() {
  let dir = tempfile::tempdir().unwrap();
  let (code, output) = run(dir.path(), "echo out; echo err >&2; cat; exit 3", "1800");
  assert_eq!((code, output.as_str()), (3, "out\nerr\n"));
  assert_eq!(run(dir.path(), "kill -TERM $$", "0").0, 128 + 15);
  assert_eq!(run(dir.path(), "sleep 0.2; exit 4", "99999999").0, 4);
}

#[test]
fn an_overrun_is_stopped_with_its_group_and_exits_124() {
  let dir = tempfile::tempdir().unwrap();
  let started = Instant::now();
  let (code, output) = run(
    dir.path(),
    "trap '' TERM; echo begun; sleep 30 & wait",
    "0.3",
  );
  assert_eq!((code, output.as_str()), (TIMEOUT_EXIT, "begun\n"));
  assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn an_invalid_bound_is_a_red_125_with_the_reason_as_output() {
  let dir = tempfile::tempdir().unwrap();
  let (code, output) = run(dir.path(), "true", "soon");
  assert_eq!(code, 125);
  assert_eq!(
    output,
    "plan-ledger: invalid PLAN_LEDGER_STEP_TIMEOUT 'soon'\n"
  );
  let missing = CheckRun {
    check: "true".to_owned(),
    cwd: dir.path().to_path_buf(),
    env: Env::from_pairs([("PATH", "/usr/bin:/bin")]),
    out_file: dir.path().join("no/such/dir/out"),
    timeout: "5".to_owned(),
  };
  assert!(run_check(&missing).is_err());
}

#[test]
fn a_bound_worktree_runs_its_check_under_a_job_lease() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  let project = root.join("project");
  std::fs::create_dir(&project).unwrap();
  assert_eq!(run(&project, "git init -q", "0").0, 0);
  let home = root.join("resources");
  std::fs::create_dir(&home).unwrap();
  std::fs::write(home.join("policy.json"), r#"{"maxJobs":1}"#).unwrap();
  bind_worktree(&home, &project, "issue-421", "/epics/402").unwrap();
  let state = home.join("state.json").display().to_string();
  let (code, output) = run(
    &project,
    &format!("grep -c '\"type\": \"job\"' {state}; echo err >&2"),
    "60",
  );
  assert_eq!((code, output.as_str()), (0, "1\nerr\n"));
  let (code, output) = run(&project, "head -c 1100000 /dev/zero | tr '\\0' x", "60");
  assert_eq!(code, 125);
  assert!(output.ends_with("plan-ledger: check output exceeded capture limit\n"));
  assert_eq!(run(&project, "sleep 5", "0.3").0, TIMEOUT_EXIT);
}

#[test]
fn timeouts_follow_gnu_timeout_durations() {
  let cases: [(&str, Option<f64>); 18] = [
    ("1800", Some(1800.0)),
    ("0", Some(0.0)),
    ("1.5", Some(1.5)),
    (".5", Some(0.5)),
    ("2m", Some(120.0)),
    ("1h", Some(3600.0)),
    ("1d", Some(86400.0)),
    ("5s", Some(5.0)),
    ("00", Some(0.0)),
    ("1e3", Some(1000.0)),
    ("+5", Some(5.0)),
    (" 5", Some(5.0)),
    ("2.5e1m", Some(1500.0)),
    ("x", None),
    ("-1", None),
    ("5 ", None),
    ("0x10", None),
    ("", None),
  ];
  for (text, seconds) in cases {
    assert_eq!(parse_timeout(text), seconds, "{text:?}");
  }
  assert_eq!(parse_timeout("1e400"), None);
  assert_eq!(parse_timeout("5."), Some(5.0));
  assert_eq!(parse_timeout("."), None);
}

#[test]
fn evidence_is_the_tail_capped_and_decoded() {
  let twelve: Vec<String> = (0..12).map(|i| format!("line {i}")).collect();
  let tail = twelve.get(2..).unwrap().join("\n");
  assert_eq!(
    evidence_of(format!("{}\n\n\n", twelve.join("\n")).as_bytes()),
    tail
  );
  assert_eq!(evidence_of(b"a\0b\n\0c\0\n"), "ab\nc");
  assert_eq!(evidence_of(b"x\r\ny\x7f\r\n"), "x\r\ny\u{7f}\r");
  assert_eq!(evidence_of(&[b'z'; 3000]), "z".repeat(2000));
  let cut = evidence_of(format!("{}é tail", "a".repeat(1999)).as_bytes());
  assert_eq!(cut, format!("{}\u{fffd}", "a".repeat(1999)));
  let invalid = [
    0x61, 0xff, 0x62, 0xc3, 0x28, 0xe2, 0x82, 0x0a, 0xf0, 0x9f, 0x98,
  ];
  assert_eq!(
    evidence_of(&invalid),
    "a\u{fffd}b\u{fffd}(\u{fffd}\n\u{fffd}"
  );
  assert_eq!(evidence_of(b"\n\n\n"), "");
}

#[test]
fn a_timed_out_step_encodes_its_reason_and_tail_again() {
  assert_eq!(step_evidence(1, b"plain\n", "7"), "plain");
  assert_eq!(
    step_evidence(TIMEOUT_EXIT, b"", "7"),
    "timed out after 7s (PLAN_LEDGER_STEP_TIMEOUT)\n\"\""
  );
  assert_eq!(
    step_evidence(TIMEOUT_EXIT, b"x\r\ny\x7f\r\n", "7"),
    "timed out after 7s (PLAN_LEDGER_STEP_TIMEOUT)\n\"x\\r\\ny\\u007f\\r\""
  );
  let long = step_evidence(TIMEOUT_EXIT, &[b'z'; 3000], "7");
  assert_eq!(long.len(), 2000);
  assert!(long.starts_with("timed out after 7s"));
}
