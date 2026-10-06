use super::{apply_patch_records, path_valid};
use crate::edit_records::EditOperation;

fn ops(patch: &str) -> Option<Vec<(String, &'static str)>> {
  let records = apply_patch_records(patch)?;
  Some(
    records
      .into_iter()
      .map(|record| (record.path, record.operation.name()))
      .collect(),
  )
}

#[test]
fn paths_are_single_line_and_tab_free() {
  assert!(path_valid("dir/a b.ts"));
  for bad in ["", "a\nb", "a\rb", "a\tb"] {
    assert!(!path_valid(bad), "{bad:?}");
  }
}

#[test]
fn every_header_yields_its_record_and_a_pending_update_flushes() {
  let patch = "*** Begin Patch\n*** Update File: u.ts\n@@\n*** Add File: n.ts\n+x\n*** End of File\n*** Update File: w.ts\n*** End Patch";
  let expected = vec![
    ("u.ts".to_owned(), "update"),
    ("n.ts".to_owned(), "add"),
    ("w.ts".to_owned(), "update"),
  ];
  assert_eq!(ops(patch), Some(expected));
}

#[test]
fn a_move_names_both_sides() {
  let records =
    apply_patch_records("*** Begin Patch\n*** Update File: a\n*** Move to: b\n*** End Patch")
      .unwrap();
  assert_eq!(records.len(), 2);
  assert_eq!(
    (records[0].operation, records[0].moved_to.as_deref()),
    (EditOperation::Update, Some("b"))
  );
  assert_eq!(
    (records[1].operation, records[1].from.as_deref()),
    (EditOperation::Move, Some("a"))
  );
}

#[test]
fn malformed_patches_yield_nothing() {
  for patch in [
    "",
    "*** Begin Patch\n*** End Patch",
    "*** Add File: a\n*** End Patch",
    "*** Begin Patch\n*** Add File: a",
    "*** Begin Patch\n*** Begin Patch\n*** Add File: a\n*** End Patch",
    "*** Begin Patch\n*** Add File: a\n*** End Patch\nx",
    "*** Begin Patch\n*** Delete File:a\n*** End Patch",
    "*** Begin Patch\n*** Update File:\n*** End Patch",
    "*** Begin Patch\n*** Move to: b\n*** End Patch",
    "*** Begin Patch\n*** Copy File: a\n*** End Patch",
    "*** End Patch",
    "*** Update File: a",
  ] {
    assert_eq!(apply_patch_records(patch), None, "{patch:?}");
  }
}
