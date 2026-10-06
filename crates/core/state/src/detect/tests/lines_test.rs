use std::path::{Path, PathBuf};

use super::{count_code_lines, count_python_code_lines, has_unterminated_block};

/// A snippet and what TypeScript's `detect-lines.ts` answers for it: the code lines,
/// the Python code lines and whether a block is left open.
type Row = (&'static str, &'static [u8], u64, u64, bool);

const ROWS: [Row; 29] = [
  (
    "blanks.ts",
    b"const a = 1;\n\n// comment\n  // indented\nconst b = 2;\n",
    2,
    4,
    false,
  ),
  (
    "multi-line-block.ts",
    b"/*\n * doc\n */\nconst a = 1;\n",
    1,
    4,
    false,
  ),
  (
    "trailing-comment.ts",
    b"const a = 1; // trailing\n",
    1,
    1,
    false,
  ),
  (
    "inline-and-rust-doc.rs",
    b"let x = /* inline */ 1;\n/// doc\n//! inner\nfn main() {}\n",
    2,
    4,
    false,
  ),
  (
    "unterminated.ts",
    b"const s = \"/* x\";\nconst a = 1;\nconst b = 2;\n",
    3,
    3,
    true,
  ),
  (
    "two-inline-blocks.ts",
    b"  /* a */ b /* c */ \n/* only */\n",
    1,
    2,
    false,
  ),
  (
    "close-before-open.ts",
    b"a */ b /* c\nd\n*/ e\n",
    2,
    3,
    false,
  ),
  ("nested-open.ts", b"/* /* */ x */\n", 1, 1, false),
  (
    "slash-star-slash.ts",
    b"/*/ still comment\n*/ x\n",
    1,
    2,
    false,
  ),
  ("crlf.ts", b"const a = 1;\r\n\r\n// c\r\n", 2, 3, false),
  (
    "no-final-newline.ts",
    b"const a = 1;\nconst b = 2;",
    2,
    2,
    false,
  ),
  ("empty.ts", b"", 0, 0, false),
  ("only-newlines.ts", b"\n\n\n", 0, 0, false),
  (
    "tabs-and-spaces.py",
    b"\t\n  # comment\n\tx = 1  # trailing\n'''doc'''\n#!shebang\n",
    4,
    2,
    false,
  ),
  (
    "docstring.py",
    b"def f():\n    \"\"\"Doc.\n\n    # not a comment line\n    \"\"\"\n    return 1\n",
    5,
    4,
    false,
  ),
  (
    "latin1-bytes.ts",
    b"const s = '\xe9\xff'; // \xe9\n/* \xe9 */\n",
    1,
    2,
    false,
  ),
  ("junction.ts", b"//* x */* y\nz\n", 2, 2, true),
  ("junction-open.ts", b"//* x */* y\n\n// c\n", 3, 2, true),
  (
    "junction-twice.ts",
    b"///**//* a */*\nb\n*/ c\n",
    2,
    3,
    false,
  ),
  ("junction-slash.ts", b"a //* x */ b\n", 1, 1, false),
  (
    "slash-block-slash.ts",
    b"/ /* x */ / b\n/* x *//* y */ q\n",
    2,
    2,
    false,
  ),
  ("unterminated-twice.ts", b"/* a\n/* b\n*/ c\n", 1, 3, true),
  ("nul.ts", b"/* /* */\0\nd\n", 2, 2, true),
  ("cr-only.ts", b"a\rb\r\n  \r\n", 2, 2, false),
  ("star-slash-first.ts", b"*/ /* x\n", 1, 1, false),
  (
    "block-then-line.ts",
    b"a /* x */ // y\nb // /* z\n",
    2,
    2,
    true,
  ),
  ("empty-block.ts", b"/**/ a\n/*/ b\n", 2, 2, false),
  ("line-comment-after-join.ts", b"/ /* x */ /b\n", 1, 1, false),
  (
    "block-joins-line-comment.ts",
    b"/* x */ / /* y */ / z\n",
    1,
    1,
    false,
  ),
];

/// `body` written to a file named `name` under `dir`.
fn file(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
  let path = dir.join(name);
  std::fs::write(&path, body).unwrap();
  path
}

#[test]
fn every_snippet_counts_as_the_typescript_counters_do() {
  let dir = tempfile::tempdir().unwrap();
  for (name, body, code, python, open) in ROWS {
    let path = file(dir.path(), name, body);
    let counted = (count_code_lines(&path), count_python_code_lines(&path));
    assert_eq!(counted, (Some(code), Some(python)), "{name}");
    assert_eq!(has_unterminated_block(&path), open, "{name}");
  }
}

#[test]
fn a_missing_path_is_unreadable_and_a_directory_is_empty() {
  let dir = tempfile::tempdir().unwrap();
  let missing = dir.path().join("missing.ts");
  assert_eq!(count_code_lines(&missing), None);
  assert_eq!(count_python_code_lines(&missing), None);
  assert!(!has_unterminated_block(&missing));
  assert_eq!(count_code_lines(dir.path()), Some(0));
  assert_eq!(count_python_code_lines(dir.path()), Some(0));
  assert!(!has_unterminated_block(dir.path()));
}

#[test]
fn a_block_open_across_many_chunks_hides_its_lines() {
  let mut body = b"/*\n".to_vec();
  body.extend(b"x\n".repeat(50_000));
  body.extend(b"*/\nconst a = 1;\n");
  let dir = tempfile::tempdir().unwrap();
  let path = file(dir.path(), "big.ts", &body);
  assert_eq!(count_code_lines(&path), Some(1));
  assert_eq!(count_python_code_lines(&path), Some(50_003));
  assert!(!has_unterminated_block(&path));
}

#[test]
fn a_line_longer_than_a_chunk_is_one_line() {
  let mut body = b"a".repeat(200_000);
  body.extend(b" /* open\nb\n");
  let dir = tempfile::tempdir().unwrap();
  let path = file(dir.path(), "long.ts", &body);
  assert_eq!(count_code_lines(&path), Some(2));
  assert!(has_unterminated_block(&path));
}
