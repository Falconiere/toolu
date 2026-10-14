use serde_json::Value;

use super::{fixer_outcome, is_claude_trust_prompt, report};
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
