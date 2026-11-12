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
  let line = format!("+{}", "x".repeat(299));
  let patch = format!("@@ -0,0 +1,8 @@\n{}\n", vec![line; 8].join("\n"));
  let mut budget = Budget::default();
  assert_eq!(budget.take(&patch).0.len(), 1);
  assert_eq!(budget.take(&patch).0.len(), 1);
  let (dropped, left) = budget.take(&patch);
  assert!(dropped.is_empty());
  assert_eq!(left, 1);
  assert!(budget.take("@@ -1 +1 @@\n-a\n+b\n").0.is_empty());
  assert_eq!(budget.dropped, 2);
}
