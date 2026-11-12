use std::fs;

use super::super::{FileOutline, Item};
use super::{entry_path, read, write};

#[test]
fn truncated_entry_is_a_miss_and_is_replaced_without_path() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let id = "a".repeat(40);
  let path = entry_path(root, "src/a.rs", &id).unwrap();
  fs::create_dir_all(path.parent().unwrap()).unwrap();
  fs::write(&path, "{\"lines\":").unwrap();
  assert!(read(root, "src/a.rs", &id).is_none());
  let file = FileOutline {
    path: "src/a.rs".to_owned(),
    lines: 1,
    items: vec![Item {
      name: "one".to_owned(),
      signature: "pub fn one()".to_owned(),
      doc: None,
      start: 1,
      end: 1,
      depth: 0,
      public: true,
    }],
  };
  write(root, &id, &file);
  assert_eq!(read(root, "other.rs", &id).unwrap().path, "other.rs");
  assert!(!fs::read_to_string(path).unwrap().contains("src/a.rs"));
}
