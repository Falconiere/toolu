//! Limits (#416 AC-4): deep nesting neither overflows a 2 MiB stack nor lets a
//! command through; long left-recursive chains are walked without recursion.

#[path = "helpers/decide.rs"]
mod decide;

use std::thread;

use toolu_shell::analysis::ShellAnalysis;
use toolu_shell::{MAX_NESTING, analyze};

/// Analyze `source` on a thread with the 2 MiB stack Rust gives test threads;
/// a stack overflow aborts the process, so returning at all is the proof.
fn on_small_stack(source: String) -> Result<ShellAnalysis, String> {
  let worker = thread::Builder::new().stack_size(2 * 1024 * 1024);
  let handle = worker
    .spawn(move || analyze(&source))
    .map_err(|err| err.to_string())?;
  handle
    .join()
    .map_err(|panic| format!("the analysis panicked: {panic:?}"))
}

#[test]
fn ten_thousand_nested_substitutions_are_unknown_not_allowed() {
  let source = format!("{}node -e x{}", "$(".repeat(10_000), ")".repeat(10_000));
  let analysis = on_small_stack(source).unwrap();
  assert!(analysis.unknown);
  let nesting = format!("nesting: deeper than {MAX_NESTING} levels");
  assert!(
    analysis.errors.iter().any(|error| error.message == nesting),
    "{:?}",
    analysis.errors
  );
  let verdict = decide::decide(&analysis, &[], &["node -e".to_owned()]);
  assert!(verdict.starts_with("unknown:"), "{verdict}");
}

#[test]
fn deep_groups_subshells_and_backticks_are_unknown_too() {
  for (open, close) in [
    ("( ", " )"),
    ("{ ", "; }"),
    ("echo \"$(", ")\""),
    ("if ", "; then :; fi"),
  ] {
    let source = format!("{}git push{}", open.repeat(2_000), close.repeat(2_000));
    let analysis = on_small_stack(source).unwrap();
    assert!(analysis.unknown, "{open}");
  }
}

#[test]
fn a_hundred_thousand_term_arithmetic_is_walked_without_recursion() {
  let source = format!("echo $(({}1)) && git push", "1+".repeat(100_000));
  let analysis = on_small_stack(source).unwrap();
  assert!(!analysis.unknown);
  assert_eq!(analysis.commands.len(), 2);
}

#[test]
fn a_hundred_thousand_and_list_keeps_every_command() {
  let source = vec!["true"; 100_000].join(" && ");
  let analysis = on_small_stack(source).unwrap();
  assert!(!analysis.unknown);
  assert_eq!(analysis.commands.len(), 100_000);
  assert!(analysis.commands.iter().all(|command| command.exit_proves));
}

#[test]
fn nesting_at_the_limit_is_still_read() {
  let depth = MAX_NESTING - 1;
  let source = format!("{}git push{}", "$(".repeat(depth), ")".repeat(depth));
  let analysis = on_small_stack(source).unwrap();
  assert!(!analysis.unknown);
  assert!(
    analysis
      .commands
      .iter()
      .any(|c| c.argv.first().cloned().flatten().as_deref() == Some("git"))
  );
}
