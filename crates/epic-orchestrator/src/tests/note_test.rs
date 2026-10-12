use super::{NOTE_CAP, bounded_note};

#[test]
fn notes_are_one_line_capped_and_redacted() {
  assert_eq!(bounded_note("a\nb"), "a b");
  assert_eq!(bounded_note("token ghp_secret"), "[redacted]");
  assert_eq!(bounded_note("github_pat_abc"), "[redacted]");
  assert_eq!(bounded_note("gho_secret"), "[redacted]");
  assert_eq!(bounded_note("Bearer secret"), "[redacted]");
  assert_eq!(bounded_note("tsak_secret"), "[redacted]");
  assert_eq!(
    bounded_note("TYPESAFE_API_KEY unset"),
    "TYPESAFE_API_KEY unset"
  );
  let long = "x".repeat(NOTE_CAP + 10);
  assert_eq!(bounded_note(&long).chars().count(), NOTE_CAP);
}

#[test]
fn screen_lines_keep_the_last_ten_non_empty_lines_capped_from_the_end() {
  let tail = (1..=14)
    .map(|n| format!("line {n}\n"))
    .collect::<Vec<_>>()
    .join("\n");
  let lines = super::screen_lines(&format!("{tail}Do you want to run rm -rf build? (y/n)  \n"));
  let kept: Vec<&str> = lines.lines().collect();
  assert_eq!(kept.len(), 10);
  assert_eq!(kept[0], "line 6");
  assert_eq!(kept[9], "Do you want to run rm -rf build? (y/n)");
  let long = super::screen_lines(&format!("{}\nend", "x".repeat(3000)));
  assert_eq!(long.chars().count(), super::NOTE_CAP);
  assert!(long.ends_with("x\nend"));
  assert_eq!(super::screen_lines("export T=ghp_secret\nok"), "[redacted]");
  assert_eq!(super::screen_lines(" \n\n"), "");
}
