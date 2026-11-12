use toolu_engine::outline::{FileOutline, Item};

use super::{MAX_TEXT, TRUNCATED, capped, dir_text, file_text};

fn item(name: &str, start: usize, depth: usize) -> Item {
  Item {
    name: name.to_owned(),
    signature: format!("pub fn {name}()"),
    doc: None,
    start,
    end: start,
    depth,
    public: true,
  }
}

fn file(path: &str, items: Vec<Item>) -> FileOutline {
  FileOutline {
    path: path.to_owned(),
    lines: items.len(),
    items,
  }
}

#[test]
fn outline_map_caps_items_per_file() {
  let many = file(
    "a.rs",
    (1..=151)
      .map(|line| item(&format!("f{line}"), line, 0))
      .collect(),
  );
  let text = file_text(&many);
  assert!(text.starts_with("a.rs: 151 lines, 151 items\n1-1 pub fn f1()"));
  assert!(text.contains("\n150-150 pub fn f150()\n... 1 more items"));
  assert!(!text.contains("f151"));
}

#[test]
fn outline_map_caps_names_per_file_and_counts_omitted_files() {
  let mut items: Vec<Item> = (1..=13)
    .map(|line| item(&format!("n{line}"), line, 0))
    .collect();
  items.push(item("method", 14, 1));
  let one = file("a.rs", items);
  let text = dir_text("src/", &[("a.rs", Some(&one)), ("min.js", None)], 1);
  assert_eq!(
    text,
    "src/: 3 files\na.rs: 14 lines, 14 items: n1, n2, n3, n4, n5, n6, n7, n8, n9, n10, n11, n12, +1 more\nmin.js: too large to outline\n... 1 more files"
  );
}

#[test]
fn outline_map_truncates_text_at_a_whole_line() {
  let line = "x".repeat(99);
  let text = vec![line; 200].join("\n");
  let short = capped(text);
  assert!(short.len() <= MAX_TEXT);
  assert!(short.ends_with(&format!("{}\n{TRUNCATED}", "x".repeat(99))));
  assert_eq!(capped("small".to_owned()), "small");
}

#[test]
fn outline_map_truncates_inside_multi_byte_text() {
  // 102-byte lines put byte 12,238, the first past the room, inside an `é`.
  let line = format!("x{}", "é".repeat(50));
  let short = capped(vec![line; 200].join("\n"));
  assert!(short.starts_with("xé"));
  assert!(short.len() <= MAX_TEXT);
  assert!(short.ends_with(TRUNCATED));
}
