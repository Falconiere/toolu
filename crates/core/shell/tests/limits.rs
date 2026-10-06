//! Limits (#416 AC-4): deep nesting neither overflows a 2 MiB stack nor lets a
//! command through; long left-recursive chains are walked without recursion.

#[path = "helpers/decide.rs"]
mod decide;

use std::thread;
use std::time::Instant;

use toolu_shell::analysis::ShellAnalysis;
use toolu_shell::{MAX_NESTING, PARSE_BUDGET, analyze};

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
fn deep_groups_subshells_quotes_and_ifs_are_unknown_too() {
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

/// tree-sitter nests an and-list 20,000 levels deep, which a recursive walk
/// could not take on 2 MiB; at 100,000 a debug build passes the 1 s budget.
#[test]
fn a_twenty_thousand_and_list_keeps_every_command() {
  let source = vec!["true"; 20_000].join(" && ");
  let analysis = on_small_stack(source).unwrap();
  assert!(!analysis.unknown, "{:?}", analysis.errors);
  assert_eq!(analysis.commands.len(), 20_000);
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

/// Whether the heredoc-state bound refused to parse.
fn refused(analysis: &ShellAnalysis) -> bool {
  analysis
    .errors
    .iter()
    .any(|error| error.message.contains("heredoc state"))
}

#[test]
fn heredocs_past_the_scanner_state_are_unknown_not_a_crash() {
  for source in [
    format!("{}\n", "cat <<EOF ".repeat(150)),
    format!("bash -c '{}'", "cat <<EOF ".repeat(150)),
    // Error recovery appends each delimiter to stale entries: this reached 1,027 bytes.
    format!("x=<<'{}'a|", "E".repeat(30)).repeat(10),
  ] {
    assert!(analyze(&source).unknown, "{:?}", source.get(..40));
  }
  let five = "cat <<EOF\nx\nEOF\n".repeat(5);
  assert!(!analyze(&five).unknown);
  // A delimiter the scanner's own check always refuses is safe.
  let huge = format!("cat <<{}\nx\n", "A".repeat(1_100));
  assert!(!refused(&analyze(&huge)));
}

/// tree-sitter-bash reads a delimiter to whitespace, `<` included; at 1,013 to
/// 1,015 characters its state passes the scanner's check and overflows the
/// 1024-byte buffer, which aborted the process before the bound (fuzz, #416).
#[test]
fn a_delimiter_in_the_scanner_overflow_window_is_unknown_not_an_abort() {
  for length in 1_000..=1_030 {
    let delimiter: String = "A<".chars().cycle().take(length).collect();
    let analysis = analyze(&format!("cat <<{delimiter}\nx\n"));
    assert_eq!(refused(&analysis), length < 1_017, "{length}");
  }
  let json = format!("gh api -X POST --input - <<< '{}'", "x".repeat(4_000));
  assert!(!analyze(&json).unknown);
}

/// tree-sitter does not check its timeout everywhere: these took 11 s and 20 s
/// to parse before long scripts moved to a worker the analysis stops waiting for.
#[test]
fn a_parse_slower_than_the_budget_is_cancelled_on_time() {
  for source in [
    "a|".repeat(20_000),
    format!("python3 - <<'EOF'\n{}\nEOF", "open('a',\"".repeat(8_000)),
  ] {
    let started = Instant::now();
    let analysis = analyze(&source);
    let elapsed = started.elapsed();
    assert!(elapsed < PARSE_BUDGET * 3, "{elapsed:?}");
    assert!(analysis.unknown, "{:?}", source.get(..20));
  }
}

/// Glued descriptors were matched against every redirect for every word.
#[test]
fn many_redirects_are_read_without_quadratic_work() {
  let source = format!("echo {}", "x >y ".repeat(32_000));
  let started = Instant::now();
  analyze(&source);
  let elapsed = started.elapsed();
  assert!(elapsed < PARSE_BUDGET * 3, "{elapsed:?}");
}
