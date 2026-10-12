use std::path::Path;

use toolu_engine::quality::EditedFile;

use super::{file_too_long, function_too_long};
use crate::file::{Limits, PyFile};

const TIGHT: Limits = Limits {
  file_lines: 3,
  fn_lines: 2,
  no_mocks: true,
};

fn edited(dir: &Path, text: &str) -> EditedFile {
  let absolute = dir.join("m.py");
  std::fs::write(&absolute, text).unwrap();
  EditedFile {
    path: "m.py".to_owned(),
    absolute,
    removed: false,
  }
}

#[test]
fn a_file_over_its_code_line_limit_is_reported_and_comments_do_not_count() {
  let dir = tempfile::tempdir().unwrap();
  let over = edited(dir.path(), "");
  let text = "a = 1\n\n# note\n\t \n  # indented\nb = 2\nc = 3\nd = 4";
  let found = file_too_long(&PyFile::new(&over, text, TIGHT));
  assert_eq!(
    found.as_deref(),
    Some(
      "File exceeds 3-line limit: m.py (4 code lines, blanks/comments excluded) — split into \
       submodules. Override via lang.python.maxFileLines."
    )
  );
  let at = "a = 1\n# c\n\nb = 2\nc = 3\n";
  assert_eq!(file_too_long(&PyFile::new(&over, at, TIGHT)), None);
  // awk reads a lone `\r` as content, so CRLF blank lines count.
  let crlf = "a = 1\r\n\r\nb = 2\r\n";
  assert!(file_too_long(&PyFile::new(&over, crlf, TIGHT)).is_none());
  let found = file_too_long(&PyFile::new(&over, &format!("{crlf}\r\n"), TIGHT));
  assert!(found.is_some_and(|text| text.contains("(4 code lines")));
}

fn long(text: &str) -> Option<String> {
  let file = EditedFile {
    path: "m.py".to_owned(),
    absolute: Path::new("/nonexistent/m.py").to_path_buf(),
    removed: false,
  };
  function_too_long(&PyFile::new(&file, text, TIGHT))
}

#[test]
fn a_function_span_closes_at_the_next_line_indented_no_deeper() {
  let text = "def a():\n    x = 1\n\n    # c\n    return x\nasync def b(y):\n    return y\n";
  assert_eq!(
    long(text).as_deref(),
    Some("Function too long in m.py (>2 lines) — extract helpers.\na:1 (3 lines)")
  );
  let nested = "class C:\n  def m(self):\n    def inner():\n      pass\n    return 1\n  x = 1\n";
  assert_eq!(
    long(nested).as_deref(),
    Some("Function too long in m.py (>2 lines) — extract helpers.\nm:2 (4 lines)")
  );
  assert_eq!(long("def a():\n    return 1\n"), None);
  let open_at_end = "def a():\n\tx = 1\n\ty = 2\n";
  assert!(long(open_at_end).unwrap().ends_with("\na:1 (3 lines)"));
  assert_eq!(long(""), None);
}
