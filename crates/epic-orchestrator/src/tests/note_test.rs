use super::{NOTE_CAP, bounded_note};

#[test]
fn notes_are_one_line_capped_and_redacted() {
  assert_eq!(bounded_note("a\nb"), "a b");
  assert_eq!(bounded_note("token ghp_secret"), "[redacted]");
  assert_eq!(bounded_note("github_pat_abc"), "[redacted]");
  assert_eq!(bounded_note("gho_secret"), "[redacted]");
  assert_eq!(bounded_note("Bearer secret"), "[redacted]");
  let long = "x".repeat(NOTE_CAP + 10);
  assert_eq!(bounded_note(&long).chars().count(), NOTE_CAP);
}
