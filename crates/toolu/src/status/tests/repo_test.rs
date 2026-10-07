use super::counts;

#[test]
fn porcelain_counts_ahead_behind_and_both_columns() {
  let text = "\
## feat/x...origin/main [ahead 1, behind 2]
M  staged
 M unstaged
MM both
?? new
";
  let found = counts(text);
  assert_eq!(found.ahead, 1);
  assert_eq!(found.behind, 2);
  assert_eq!(found.staged, 2);
  assert_eq!(found.unstaged, 2);
  assert_eq!(found.untracked, 1);
  let bare = counts("## feat/x\n");
  assert_eq!(bare.ahead, 0);
  assert_eq!(bare.staged, 0);
}
