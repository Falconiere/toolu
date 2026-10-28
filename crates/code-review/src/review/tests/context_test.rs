use super::enclosing;
use crate::contract::Side;
use crate::shape::{ChangedLine, FileKind, ShapedFile};

/// A modified file whose added new-side lines are `added`.
fn file(added: &[u32]) -> ShapedFile {
  ShapedFile {
    path: "src/m.py".to_owned(),
    old_path: None,
    kind: FileKind::Modified,
    patch: String::new(),
    lines: added
      .iter()
      .map(|line| ChangedLine {
        side: Side::New,
        line: *line,
        text: String::new(),
      })
      .collect(),
  }
}

const TEXT: &str = "import os\n\ndef first(a):\n    return a\n\nclass Second:\n    def third(self):\n        return 1\n";

#[test]
fn each_change_names_the_nearest_definition_at_or_above_it() {
  let mut notes = Vec::new();
  let names = enclosing(&file(&[4, 6, 8]), TEXT, &mut notes);
  let found: Vec<(&str, u32)> = names
    .iter()
    .map(|(name, line)| (name.as_str(), *line))
    .collect();
  assert_eq!(found, [("Second", 6), ("first", 3), ("third", 7)]);
  assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_change_above_every_definition_names_nothing() {
  let mut notes = Vec::new();
  assert!(enclosing(&file(&[1]), TEXT, &mut notes).is_empty());
}
