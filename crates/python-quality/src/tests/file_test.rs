use std::path::PathBuf;

use toolu_engine::quality::EditedFile;

use super::{Limits, PyFile, base_name, ends_with_py, head, is_python_test_name, split_lines};

#[test]
fn lines_are_read_as_awk_reads_them() {
  assert_eq!(split_lines("").len(), 0);
  assert_eq!(split_lines("a\nb\n"), ["a", "b"]);
  assert_eq!(split_lines("a\r\n\nb"), ["a\r", "", "b"]);
  let file = EditedFile {
    path: "x.py".to_owned(),
    absolute: PathBuf::from("/x.py"),
    removed: false,
  };
  let f = PyFile::new(&file, "def f():\n", Limits::default());
  assert_eq!((f.lines.len(), f.limits.file_lines), (1, 400));
}

#[test]
fn head_and_names_follow_the_shell_idioms() {
  let items: Vec<String> = ["1", "2", "3"].map(str::to_owned).to_vec();
  assert_eq!(head(&items, 2), "1\n2");
  assert_eq!(head(&items, 5), "1\n2\n3");
  for (name, test) in [
    ("test_a.py", true),
    ("a_test.py", true),
    ("_test.py", true),
    ("test_a.PY", false),
    ("testa.py", false),
    ("a.py", false),
  ] {
    assert_eq!(is_python_test_name(name), test, "{name}");
  }
  assert!(ends_with_py(".py") && !ends_with_py("a.Py") && !ends_with_py("apy"));
  assert_eq!(base_name("src/pkg/test_a.py"), "test_a.py");
  assert_eq!(base_name("a.py"), "a.py");
}
