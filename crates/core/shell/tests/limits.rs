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

/// tree-sitter nests an and-list 10,000 levels deep, which a recursive walk
/// could not take on 2 MiB. Sized well inside the 1 s budget of a debug,
/// coverage-instrumented build; release walks 100,000 terms within it.
#[test]
fn a_ten_thousand_and_list_keeps_every_command() {
  let source = vec!["true"; 10_000].join(" && ");
  let analysis = on_small_stack(source).unwrap();
  assert!(!analysis.unknown, "{:?}", analysis.errors);
  assert_eq!(analysis.commands.len(), 10_000);
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

/// Every input that aborted tree-sitter-bash 0.23.3's heredoc serializer (fuzzing
/// and review of #416) now parses: the vendored scanner has the 0.25.1 check.
/// Returning at all is the proof, as an abort kills the test process.
#[test]
fn heredoc_state_that_overflowed_the_scanner_is_parsed_without_a_crash() {
  let mut sources = vec![
    format!("{}\n", "cat <<EOF ".repeat(150)),
    format!("bash -c '{}'", "cat <<EOF ".repeat(150)),
    format!("cat <<<<<{}", "a".repeat(503)),
    // Error recovery appends each delimiter to stale entries: this reached 1,027 bytes.
    format!("x=<<'{}'a|", "E".repeat(30)).repeat(10),
  ];
  for length in 1_000..=1_030 {
    let delimiter: String = "A<".chars().cycle().take(length).collect();
    sources.push(format!("cat <<{delimiter}\nx\n"));
  }
  for length in 1_005..=1_011 {
    sources.push(format!("\"\"''\"\"\"\\\"\"\\<<'E'{{{}", "a".repeat(length)));
  }
  // Each is malformed bash: unknown, with what it read still reported.
  for source in &sources {
    let analysis = analyze(source);
    assert!(analysis.unknown, "{:?}", source.get(..30));
    assert!(!analysis.commands.is_empty(), "{:?}", source.get(..30));
  }
  // Ten heredocs, one file each, are an ordinary command.
  let files: Vec<String> = (0..10)
    .map(|n| format!("cat > f{n} <<'EOF'\nx\nEOF\n"))
    .collect();
  let files = files.join("");
  let analysis = analyze(&files);
  assert!(!analysis.unknown, "{:?}", analysis.errors);
  assert_eq!(analysis.commands.len(), 10);
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

/// Glued descriptors were matched against every redirect for every word:
/// 32,000 redirects took 4.6 s. 8,000 now stay well inside the budget.
#[test]
fn many_redirects_are_read_without_quadratic_work() {
  let analysis = analyze(&format!("echo {}", "x >y ".repeat(8_000)));
  assert!(!analysis.unknown, "{:?}", analysis.errors);
  let redirects: Vec<usize> = analysis
    .commands
    .iter()
    .map(|command| command.redirects.len())
    .collect();
  assert_eq!(redirects, [8_000]);
}

/// The fixups looked up each `{`'s parent from the root: 40,000 nested groups took 37 s.
#[test]
fn nested_groups_are_fixed_up_without_quadratic_work() {
  let source = format!("{}a{}", "{ ".repeat(40_000), "; }".repeat(40_000));
  let analysis = analyze(&source);
  let messages: Vec<&str> = analysis
    .errors
    .iter()
    .map(|error| error.message.as_str())
    .collect();
  assert_eq!(
    messages,
    [format!("nesting: deeper than {MAX_NESTING} levels")]
  );
}
