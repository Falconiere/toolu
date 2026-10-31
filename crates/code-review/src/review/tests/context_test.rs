use regex::Regex;

use super::{DEFINITION, enclosing};
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

fn pattern() -> Regex {
  Regex::new(DEFINITION).unwrap()
}

const TEXT: &str = "import os\n\ndef first(a):\n    return a\n\nclass Second:\n    def third(self):\n        return 1\n";

#[test]
fn each_change_names_the_nearest_definition_at_or_above_it() {
  let names = enclosing(&pattern(), &file(&[4, 6, 8]), TEXT);
  let found: Vec<(&str, u32)> = names
    .iter()
    .map(|(name, line)| (name.as_str(), *line))
    .collect();
  assert_eq!(found, [("Second", 6), ("first", 3), ("third", 7)]);
}

#[test]
fn a_change_above_every_definition_names_nothing() {
  assert!(enclosing(&pattern(), &file(&[1]), TEXT).is_empty());
}
