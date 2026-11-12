use super::super::{Digest, FileDigest, empty};
use super::text;
use crate::contract::Omitted;
use crate::shape::FileKind;

fn renamed() -> FileDigest {
  FileDigest {
    path: "src/new.rs".to_owned(),
    old_path: Some("src/old.rs".to_owned()),
    kind: FileKind::Renamed,
    added: 1,
    removed: 1,
    items: Vec::new(),
    items_omitted: 0,
    removed_items: Vec::new(),
    hunks: Vec::new(),
    hunks_omitted: 0,
  }
}

#[test]
fn an_empty_digest_says_no_changes_against_its_base() {
  let digest = empty("origin/main".to_owned(), "abc".to_owned());
  assert_eq!(text(&digest), "No changes against origin/main");
}

#[test]
fn a_digest_names_renames_unavailable_items_omitted_files_and_unread_paths() {
  let digest = Digest {
    added: 1,
    removed: 1,
    files: vec![renamed()],
    files_omitted: 2,
    items_unavailable: Some("ast-grep not found on PATH".to_owned()),
    unread: vec![Omitted {
      path: "big.json".to_owned(),
      reason: "diff exceeds one whole-file evidence chunk".to_owned(),
    }],
    ..empty("main".to_owned(), "0123456789".to_owned())
  };
  assert_eq!(
    text(&digest),
    "digest: main (merge-base 0123456), 3 files, +1 -1
note: items unavailable: ast-grep not found on PATH
src/new.rs (from src/old.rs) +1 -1 (renamed)
  items unavailable
... 2 more files; pass --path to narrow
unread: big.json (diff exceeds one whole-file evidence chunk)"
  );
}
