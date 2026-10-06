//! The heredoc-state bound: one push per run of `<`, every word a `<<` token
//! can append (with its NUL) at any alignment, times the heredoc count (at
//! least `RELEX`), and the limit.

use super::{HUGE, RELEX, SCANNER_STATE_LIMIT, heredoc_state};

fn bound(bytes: usize) -> usize {
  4 + RELEX * bytes
}

#[test]
fn a_command_without_heredocs_needs_only_the_header() {
  assert_eq!(heredoc_state("git push"), 4);
  assert_eq!(heredoc_state("sort < in > out"), 4);
}

#[test]
fn each_heredoc_costs_seven_bytes_its_delimiter_and_a_nul() {
  assert_eq!(heredoc_state("cat <<EOF"), bound(7 + 4));
  // `<<-'END'` reads `-'END'` unquoted, or `END` after the dash.
  assert_eq!(
    heredoc_state("cat <<EOF >x <<-'END'"),
    bound(11 + 7 + 7 + 4)
  );
  assert_eq!(heredoc_state("cat <<'a b c'"), bound(7 + 6));
  assert_eq!(heredoc_state("cat <<a\\ b"), bound(7 + 4));
  assert_eq!(heredoc_state("cat <<ab\0cd"), bound(7 + 3));
}

#[test]
fn a_delimiter_runs_to_whitespace_through_redirect_characters() {
  assert_eq!(heredoc_state("cat <<EOF>out"), bound(7 + 8));
  let window = format!("cat <<{}\nx\n", "A<".repeat(507));
  assert!(heredoc_state(&window) >= SCANNER_STATE_LIMIT);
}

#[test]
fn every_alignment_of_a_run_of_angle_brackets_counts() {
  // `<<<<<<<<5`: the pairs before the last read `<…<5`, the last reads `5`.
  let inner: usize = (1..=6).map(|left| left + 2).sum();
  assert_eq!(heredoc_state("<<<<<<<<5"), bound(7 + inner + 2));
  assert_eq!(heredoc_state("cat \\<<<'AAA'"), bound(7 + 7 + 4));
  assert_eq!(heredoc_state("cat <<< x"), bound(7 + 2 + 2));
}

#[test]
fn a_delimiter_sure_to_fail_the_scanner_check_counts_nothing() {
  let json = format!("gh api -X POST --input - <<< '{}'", "x".repeat(4_000));
  assert_eq!(heredoc_state(&json), bound(7 + 2));
  let long = format!("cat <<{}", "A".repeat(HUGE));
  assert_eq!(heredoc_state(&long), bound(7));
  let one_less = format!("cat <<{}", "A".repeat(HUGE - 1));
  assert!(heredoc_state(&one_less) >= SCANNER_STATE_LIMIT);
}

#[test]
fn words_count_only_once_something_pushes() {
  assert_eq!(heredoc_state("(( a <<= 1 ))"), 4);
  assert_eq!(heredoc_state("(( a <<= 1 )); cat <<EOF"), bound(2 + 7 + 4));
}

#[test]
fn whitespace_the_c_library_may_keep_makes_no_length_sure() {
  assert_eq!(heredoc_state("cat <<\u{2003}\"a b\""), bound(7 + 5));
  // A word with non-ASCII whitespace in it may end there: it is not sure to be huge.
  let spaced = format!("cat <<{}\u{2003}{}", "A".repeat(500), "B".repeat(600));
  assert!(heredoc_state(&spaced) >= SCANNER_STATE_LIMIT);
}

#[test]
fn more_heredocs_multiply_the_state_error_recovery_can_reach() {
  // Five heredocs: five pushes and words, each assumed to reach the stack five times.
  assert_eq!(
    heredoc_state(&"cat <<EOF\nx\nEOF\n".repeat(5)),
    4 + 5 * 5 * 11
  );
  // Measured: this reached 1,027 bytes, with stale entries appended again.
  let recovered = format!("x=<<'{}'a|", "E".repeat(30)).repeat(10);
  assert!(heredoc_state(&recovered) >= SCANNER_STATE_LIMIT);
}

#[test]
fn counting_stops_at_the_limit_and_stays_linear() {
  assert!(heredoc_state(&"cat <<EOF ".repeat(150)) >= SCANNER_STATE_LIMIT);
  let started = std::time::Instant::now();
  // `<<=` pushes nothing, and each word runs to the end: nothing can be stored.
  assert_eq!(heredoc_state(&"<<=a".repeat(250_000)), 4);
  assert!(heredoc_state(&"<".repeat(1_000_000)) >= SCANNER_STATE_LIMIT);
  let elapsed = started.elapsed();
  assert!(elapsed < std::time::Duration::from_secs(2), "{elapsed:?}");
}
