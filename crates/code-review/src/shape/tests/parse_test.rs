use crate::contract::Side;

use super::{patch_lines, statuses};
use crate::shape::FileKind;

#[test]
fn parse_preserves_rename_paths_and_changed_side_lines() {
  let status = statuses(b"R090\0old name.rs\0new name.rs\0").expect("status");
  assert_eq!(status.len(), 1);
  assert_eq!(status[0].kind, FileKind::Renamed);
  assert_eq!(status[0].old_path.as_deref(), Some("old name.rs"));
  assert_eq!(status[0].path, "new name.rs");
  let lines =
    patch_lines("@@ -4 +4 @@\n-old\n+new\n@@ -8,0 +9 @@\n+added\n").expect("changed lines");
  assert_eq!(
    lines
      .iter()
      .map(|line| (line.side, line.line))
      .collect::<Vec<_>>(),
    [(Side::Old, 4), (Side::New, 4), (Side::New, 9)]
  );
}
