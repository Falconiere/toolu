use super::windows;
use crate::contract::Side;
use crate::shape::{ChangedLine, FileKind, ShapedFile};

/// Lines `l{n}` for each `n` in `numbers`, each newline-terminated.
fn numbered(numbers: impl Iterator<Item = usize>) -> String {
  numbers
    .map(|line| format!("l{line}\n"))
    .collect::<Vec<_>>()
    .concat()
}

/// A modified file whose `patch` holds `hunks` and whose added lines are `added`.
fn file(hunks: &[&str], added: &[u32]) -> ShapedFile {
  ShapedFile {
    path: "src/big.rs".to_owned(),
    old_path: None,
    kind: FileKind::Modified,
    patch: format!("{}\n", hunks.join("\n")),
    lines: added
      .iter()
      .map(|line| ChangedLine {
        side: Side::New,
        line: *line,
        text: format!("l{line}"),
      })
      .collect(),
  }
}

#[test]
fn one_change_sends_the_lines_around_it_verbatim_under_a_header() {
  let (out, sent, total) = windows(
    &numbered(1..=200),
    &file(&["@@ -100 +100 @@"], &[100]),
    1 << 20,
  );
  assert_eq!((sent, total), (1, 1));
  let lines = numbered(70..=130);
  assert_eq!(out, format!("@@ lines 70-130 @@\n{lines}"));
}

#[test]
fn nearby_changes_merge_and_distant_ones_get_their_own_window() {
  let near = file(&["@@ -100 +100 @@", "@@ -120 +120 @@"], &[100, 120]);
  let (out, sent, total) = windows(&numbered(1..=400), &near, 1 << 20);
  assert_eq!((sent, total), (1, 1));
  assert!(out.starts_with("@@ lines 70-150 @@\n"), "{out}");
  let far = file(&["@@ -100 +100 @@", "@@ -300 +300 @@"], &[100, 300]);
  let (out, sent, total) = windows(&numbered(1..=400), &far, 1 << 20);
  assert_eq!((sent, total), (2, 2));
  assert!(out.contains("@@ lines 270-330 @@\nl270\n"), "{out}");
}

#[test]
fn windows_clamp_to_the_file_and_a_pure_deletion_still_gets_one() {
  // `+5,0`: lines were removed after new line 5; no added line marks the spot.
  let (out, sent, _) = windows(&numbered(1..=40), &file(&["@@ -6,2 +5,0 @@"], &[]), 1 << 20);
  assert_eq!(sent, 1);
  assert!(out.starts_with("@@ lines 1-35 @@\nl1\n"), "{out}");
  assert!(out.ends_with("l35\n"), "{out}");
}

#[test]
fn a_window_that_would_pass_the_cap_is_left_out_whole() {
  let far = file(&["@@ -100 +100 @@", "@@ -300 +300 @@"], &[100, 300]);
  let (all, ..) = windows(&numbered(1..=400), &far, 1 << 20);
  let first = all.find("@@ lines 270").unwrap();
  // One byte short of both windows: only the first fits, and it is whole.
  let (out, sent, total) = windows(&numbered(1..=400), &far, all.len() - 1);
  assert_eq!((sent, total), (1, 2));
  assert_eq!(Some(out.as_str()), all.get(..first));
  let (out, sent, _) = windows(&numbered(1..=400), &far, 10);
  assert_eq!((out.as_str(), sent), ("", 0));
}

#[test]
fn a_last_line_without_a_newline_still_ends_its_window() {
  let (out, _, _) = windows("a\nb\nc", &file(&["@@ -3 +3 @@"], &[3]), 1 << 20);
  assert_eq!(out, "@@ lines 1-3 @@\na\nb\nc\n");
}
