use super::{Budget, remaining};

#[test]
fn remaining_names_the_unshown_old_and_new_lines() {
  assert_eq!(remaining("@@ -0,0 +1,26 @@").as_deref(), Some("new 9-26"));
  assert_eq!(
    remaining("@@ -10,12 +9,3 @@").as_deref(),
    Some("old 18-21, new 9-11")
  );
  assert_eq!(remaining("@@ -4,5 +4,5 @@").as_deref(), Some("new 7-8"));
  assert_eq!(remaining("@@ -4 +4 @@").as_deref(), Some(""));
  assert_eq!(remaining("@@ bad @@"), None);
}

#[test]
fn a_file_past_the_budget_and_every_later_one_print_no_hunks() {
  let line = format!("+{}", "x".repeat(199));
  let patch = format!("@@ -0,0 +1,8 @@\n{}\n", vec![line; 8].join("\n"));
  let mut budget = Budget::default();
  for _ in 0..3 {
    assert_eq!(budget.take(&patch).0.len(), 1);
  }
  let (dropped, left) = budget.take(&patch);
  assert!(dropped.is_empty());
  assert_eq!(left, 1);
  assert!(budget.take("@@ -1 +1 @@\n-a\n+b\n").0.is_empty());
  assert_eq!(budget.dropped, 2);
}

#[test]
fn a_long_line_is_cut_so_one_file_cannot_spend_the_budget() {
  let patch = format!("@@ -0,0 +1 @@\n+{}\n", "\u{1F600}".repeat(7000));
  let mut budget = Budget::default();
  let (hunks, _) = budget.take(&patch);
  let line = &hunks[0].lines[0];
  // `+` and 49 four-byte characters fit in 200 bytes.
  assert_eq!(line.len(), 1 + 49 * 4 + 3);
  assert!(line.ends_with("..."));
  assert_eq!(budget.dropped, 0);
}
