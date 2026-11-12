use super::super::{Definition, Found, Reference};
use super::{cut, render};

fn reference(path: &str, line: usize) -> Reference {
  Reference {
    path: path.to_owned(),
    line,
    text: format!("go({line})"),
    within: Some(("main".to_owned(), 1, 99)),
  }
}

fn found(references: Vec<Reference>) -> Found {
  let files = {
    let mut paths: Vec<&str> = references.iter().map(|one| one.path.as_str()).collect();
    paths.dedup();
    paths.len() + 1
  };
  Found {
    name: "go".to_owned(),
    definitions: vec![Definition {
      path: "a.rs".to_owned(),
      line: 2,
      start: 1,
      end: 3,
      signature: "fn go()".to_owned(),
    }],
    references,
    files,
    ..Found::default()
  }
}

#[test]
fn symbol_render_prints_definitions_then_references_by_file() {
  let mut one = found(vec![reference("b.rs", 4)]);
  one.references.push(Reference {
    path: "c.py".to_owned(),
    line: 1,
    text: "go()".to_owned(),
    within: None,
  });
  one.files = 3;
  assert_eq!(
    render(&one, false),
    "go: 1 definition, 2 references in 3 files\ndef a.rs:1-3 fn go()\nb.rs\n  4 in main 1-99: go(4)\nc.py\n  1: go()"
  );
}

#[test]
fn symbol_render_caps_eight_per_file_and_forty_in_all() {
  let refs = (0..6)
    .flat_map(|file| (1..=10).map(move |line| reference(&format!("f{file}.rs"), line)))
    .collect();
  let text = render(&found(refs), false);
  let lines: Vec<&str> = text.lines().collect();
  assert_eq!(
    lines.first(),
    Some(&"go: 1 definition, 60 references in 7 files")
  );
  assert_eq!(
    lines
      .iter()
      .filter(|line| line.contains(" in main "))
      .count(),
    40
  );
  assert_eq!(
    lines
      .iter()
      .filter(|line| line.contains("... 2 more in this file"))
      .count(),
    5
  );
  assert!(!text.contains("f5.rs"));
  assert_eq!(
    lines.last(),
    Some(&"Narrow: toolu ast-grep symbol go <path>")
  );
  let nine: Vec<Reference> = (1..=9).map(|line| reference("b.rs", line)).collect();
  assert!(
    render(&found(nine), false)
      .ends_with("  ... 1 more in this file\nNarrow: toolu ast-grep symbol go <path>")
  );
}

#[test]
fn symbol_render_notes_missing_ast_grep_and_marks_partial_output() {
  let mut one = found(vec![reference("b.rs", 4)]);
  one.missing = true;
  one.partial = true;
  let text = render(&one, false);
  assert!(
    text.starts_with("go: 1 definition, 1 reference in 2 files (grep output capped at 256 KiB)\n")
  );
  assert!(text.contains("\nnote: ast-grep not found on PATH; definitions and enclosing items are skipped (install: brew install ast-grep)\nNarrow:"));
  let doc: serde_json::Value = serde_json::from_str(&render(&one, true)).unwrap();
  assert_eq!(doc["partial"], true);
  assert_eq!(doc["truncated"], true);
  assert!(
    doc["note"]
      .as_str()
      .unwrap()
      .starts_with("ast-grep not found")
  );
}

#[test]
fn symbol_render_cuts_lines_at_120_characters() {
  assert_eq!(cut(&"x".repeat(120)), "x".repeat(120));
  let long = cut(&"é".repeat(130));
  assert_eq!(long.chars().count(), 120);
  assert!(long.ends_with("..."));
}
