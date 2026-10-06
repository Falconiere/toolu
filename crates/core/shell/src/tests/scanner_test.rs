//! The heredoc-state bound: one push per run of `<`, the delimiter as
//! tree-sitter-bash's `advance_word` reads it (with its NUL), and the limit.

use super::{SCANNER_STATE_LIMIT, heredoc_state};

#[test]
fn a_command_without_heredocs_needs_only_the_header() {
  assert_eq!(heredoc_state("git push"), (0, 4));
  assert_eq!(heredoc_state("sort < in > out"), (0, 4));
}

#[test]
fn each_heredoc_costs_seven_bytes_its_delimiter_and_a_nul() {
  // `EOF` is 7 + 1 + 3; `<<-'END'` is 7, the dash, then `'END'` read unquoted.
  assert_eq!(heredoc_state("cat <<EOF >x <<-'END'"), (2, 4 + 11 + 14));
  assert_eq!(heredoc_state("cat <<'a b c'"), (1, 4 + 7 + 1 + 5));
  assert_eq!(heredoc_state("cat <<a\\ b"), (1, 4 + 7 + 1 + 3));
  assert_eq!(heredoc_state("cat <<ab\0cd"), (1, 4 + 7 + 1 + 2));
}

#[test]
fn a_delimiter_runs_to_whitespace_through_redirect_characters() {
  assert_eq!(heredoc_state("cat <<EOF>out"), (1, 4 + 7 + 1 + 7));
  let long = format!("cat <<{}\nx\n", "A<".repeat(600));
  assert!(heredoc_state(&long).1 >= SCANNER_STATE_LIMIT);
}

#[test]
fn a_here_string_pushes_a_heredoc_but_appends_no_delimiter() {
  assert_eq!(heredoc_state("cat <<< x"), (1, 11));
  let json = format!("gh api -X POST --input - <<< '{}'", "x".repeat(4_000));
  assert_eq!(heredoc_state(&json), (1, 11));
}

#[test]
fn runs_of_angle_brackets_push_once_and_read_a_word_after_a_trailing_pair() {
  assert_eq!(heredoc_state("<<<<"), (1, 11));
  assert_eq!(heredoc_state("<<<<<<<<5"), (1, 4 + 7 + 1 + 1));
  assert_eq!(heredoc_state("cat \\<<<'AAA'"), (1, 4 + 7 + 1 + 5));
  assert_eq!(heredoc_state("cat \\\\<<<x"), (1, 11));
}

#[test]
fn shift_assignment_pushes_nothing_but_its_word_still_counts() {
  assert_eq!(heredoc_state("(( a <<= 1 ))"), (0, 4 + 1 + 1));
}

#[test]
fn whitespace_the_c_library_may_keep_counts_toward_the_delimiter() {
  assert_eq!(
    heredoc_state("cat <<\u{2003}\"a b\""),
    (1, 4 + 7 + 1 + 1 + 3)
  );
}

#[test]
fn counting_stops_at_the_limit() {
  let (count, size) = heredoc_state(&"cat <<EOF ".repeat(150));
  assert!(size >= SCANNER_STATE_LIMIT);
  assert!(count < 150, "{count}");
  let (_, long) = heredoc_state(&format!("cat <<{}", "A".repeat(5_000)));
  assert_eq!(long, 4 + 7 + 1 + SCANNER_STATE_LIMIT);
}
