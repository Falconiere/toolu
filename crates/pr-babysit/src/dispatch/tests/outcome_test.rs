use std::path::Path;

use serde_json::Value;

use super::{fixer_outcome, host_errors, is_claude_trust_prompt, log_tail, report};
use crate::fixture;

const TRUSTED: &str = "/Users/falconiere/.herdr/worktrees/probe/pb-probe";

#[test]
fn a_report_wins_over_the_screen_and_a_limit_wins_over_silence() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("report.json");
  std::fs::write(&file, "{\"status\":\"done\"}").unwrap();
  assert_eq!(fixer_outcome(&file, "rate limit"), "done");
  std::fs::write(&file, "{\"status\":\"failed\"}").unwrap();
  assert_eq!(fixer_outcome(&file, ""), "reported_failed");
  std::fs::write(&file, "{").unwrap();
  assert_eq!(fixer_outcome(&file, "rate limit 429"), "host_limited");
  assert_eq!(
    fixer_outcome(&file, "You've hit your usage LIMIT"),
    "host_limited"
  );
  assert_eq!(fixer_outcome(&file, "tests passed"), "no_report");
}

#[test]
fn only_the_plain_trust_prompt_for_this_worktree_is_accepted() {
  let pane = fixture::text("herdr/claude-trust-prompt.txt");
  assert!(is_claude_trust_prompt(&pane, TRUSTED));
  assert!(!is_claude_trust_prompt(&pane, &format!("{TRUSTED}-other")));
  let extras = pane.replace("No, exit", "No, continue without these permissions");
  assert!(!is_claude_trust_prompt(&extras, TRUSTED));
  assert!(!is_claude_trust_prompt("Yes, I trust this folder", TRUSTED));
}

#[test]
fn limit_detection_reads_only_the_host_errors_of_a_captured_log() {
  let tail = fixture::text("opencode/fixer-r1g1.log");
  let expected = "timestamp=2026-10-03T20:29:09.660Z level=ERROR run=dddd81cc message=\"cli process failed\" cause=\"Cause([Fail(~effect/cli/CliError/ShowHelp: Help requested)])\" role=cli";
  assert_eq!(host_errors(&tail), expected);
  let none = Path::new("/nonexistent/report.json");
  let noisy = format!(
    "{tail}{}\n{}\n",
    r#"{"type":"tool_use","part":{"tool":"bash","state":{"status":"completed","output":"429 Too Many Requests: rate limit"}}}"#,
    "timestamp=2026-10-03T20:29:40.000Z level=INFO message=evaluated pattern=\"grep -r 'rate limit' src\""
  );
  assert_eq!(fixer_outcome(none, &host_errors(&noisy)), "no_report");
  let limited = format!(
    "{tail}{}\n",
    r#"{"type":"error","error":{"name":"APIError","data":{"message":"429 Too Many Requests"}}}"#
  );
  assert_eq!(fixer_outcome(none, &host_errors(&limited)), "host_limited");
}

#[test]
fn the_tail_never_starts_with_a_cut_off_line() {
  let dir = tempfile::tempdir().unwrap();
  let log = dir.path().join("fixer.log");
  let lines: Vec<String> = (0..5000)
    .map(|index| format!("line {index} {}", "x".repeat(40)))
    .collect();
  std::fs::write(&log, format!("{}\n", lines.join("\n"))).unwrap();
  assert_eq!(
    log_tail(&log, 3),
    format!("{}\n{}\n", lines[4998], lines[4999])
  );
  assert_eq!(log_tail(&log, 40).split('\n').next().unwrap(), lines[4961]);
  let all: Vec<String> = log_tail(&log, 100_000)
    .split('\n')
    .map(str::to_owned)
    .collect();
  assert!(lines.contains(&all[0]));
  assert!(all.len() < lines.len());
  assert_eq!(log_tail(&dir.path().join("missing.log"), 40), "");
}

#[test]
fn the_report_is_written_whole_with_its_note() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("owner's.report.json");
  report(&file, "done", Some("fixed"), "2026-10-11T12:00:00Z").unwrap();
  let written: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
  assert_eq!(written["status"], "done");
  assert_eq!(written["note"], "fixed");
  report(&file, "failed", None, "2026-10-11T12:00:00Z").unwrap();
  assert_eq!(fixer_outcome(&file, ""), "reported_failed");
  assert_eq!(
    report(&file, "maybe", None, "x").err().unwrap().code,
    "usage"
  );
}
