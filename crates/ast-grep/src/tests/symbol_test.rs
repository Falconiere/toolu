use toolu_engine::outline::{FileOutline, Item, supported};

use super::{Hit, PATHSPEC, Reference, classify, grep_argv, on_signature, parse};

fn item(name: &str, signature: &str, start: usize, end: usize, depth: usize) -> Item {
  Item {
    name: name.to_owned(),
    signature: signature.to_owned(),
    doc: None,
    start,
    end,
    depth,
    public: true,
  }
}

fn hit(path: &str, line: usize, text: &str) -> Hit {
  Hit {
    path: path.to_owned(),
    line,
    text: text.to_owned(),
  }
}

#[test]
fn symbol_pathspec_names_only_outlined_languages() {
  assert!(
    PATHSPEC
      .iter()
      .all(|spec| supported(&spec.replace('*', "x")))
  );
  let argv = grep_argv("-x", &[]);
  assert_eq!(
    argv.get(8..11),
    Some(&["-e".to_owned(), "-x".to_owned(), "--".to_owned()][..])
  );
  assert_eq!(
    grep_argv("x", &["a b".to_owned()])
      .last()
      .map(String::as_str),
    Some("a b")
  );
}

#[test]
fn symbol_parse_reads_nul_fields_and_drops_a_cut_last_line() {
  let out = b"src/a:b.rs\x003\x00  let y = add(1);\nsrc/c.rs\x0012\x00add(";
  assert_eq!(
    parse(out, true),
    vec![hit("src/a:b.rs", 3, "let y = add(1);")]
  );
  assert_eq!(parse(out, false).len(), 2);
  assert_eq!(parse(b"no nul here\n", false), Vec::new());
}

#[test]
fn symbol_signature_line_matches_prefix_either_way() {
  assert!(on_signature(
    "pub fn add(x: i32) -> i32 {",
    "pub fn add(x: i32) -> i32"
  ));
  assert!(on_signature("pub fn add(", "pub fn add(x: i32) -> i32"));
  assert!(on_signature("pub  fn   add(x)", "pub fn add(x) where ..."));
  let long = format!("pub fn add({}", "y".repeat(200));
  let capped = format!("{}...", long.get(..157).unwrap_or_default());
  assert!(on_signature(&long, &capped));
  assert!(!on_signature(
    "/// add adds one",
    "pub fn add(x: i32) -> i32"
  ));
  assert!(!on_signature("add(x - 1)", "pub fn add(x: i32) -> i32"));
}

#[test]
fn symbol_classify_splits_definitions_from_enclosed_references() {
  let outlines = vec![FileOutline {
    path: "a.rs".to_owned(),
    lines: 12,
    items: vec![
      item("add", "pub fn add(x: i32) -> i32", 1, 5, 0),
      item("Add", "impl Add", 7, 11, 0),
      item("main", "fn main()", 8, 10, 1),
    ],
  }];
  let hits = vec![
    hit("a.rs", 1, "/// add one"),
    hit("a.rs", 2, "pub fn add(x: i32) -> i32 {"),
    hit("a.rs", 4, "add(x - 1)"),
    hit("a.rs", 9, "add(2);"),
    hit("a.rs", 12, "add(3);"),
    hit("notes.md", 1, "call add"),
  ];
  let (defs, refs) = classify("add", hits, &outlines);
  assert_eq!(defs.len(), 1);
  assert_eq!((defs[0].line, defs[0].start, defs[0].end), (2, 1, 5));
  let within: Vec<Option<&str>> = refs
    .iter()
    .map(|one: &Reference| one.within.as_ref().map(|(name, _, _)| name.as_str()))
    .collect();
  assert_eq!(
    within,
    vec![Some("add"), Some("add"), Some("main"), None, None]
  );
}
