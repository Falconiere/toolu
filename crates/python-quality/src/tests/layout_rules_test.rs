use std::path::Path;

use toolu_engine::quality::EditedFile;

use super::test_layout;
use crate::file::{Limits, PyFile};

fn layout(dir: &Path, name: &str, text: &str) -> Vec<String> {
  let absolute = dir.join(name);
  std::fs::write(&absolute, text).unwrap();
  let file = EditedFile {
    path: format!("pkg/{name}"),
    absolute,
    removed: false,
  };
  test_layout(&PyFile::new(&file, text, Limits::default()))
}

const MISNAMED: &str = "Test-bearing file not named test_*.py or *_test.py: pkg/";
const ALONE: &str = "Test not co-located with a module: pkg/";

#[test]
fn a_test_marker_needs_the_test_naming_convention() {
  let dir = tempfile::tempdir().unwrap();
  for marker in [
    "def test_a():\n",
    "async  def test_b (x):\n",
    "import pytest\n",
    "from unittest import TestCase\n",
  ] {
    let errors = layout(dir.path(), "helpers.py", marker);
    assert_eq!(errors.len(), 1, "{marker:?}");
    assert!(errors[0].starts_with(&format!("{MISNAMED}helpers.py — rename")));
  }
  for quiet in [
    "import pytest_mock\n",
    "    def test_a():\n",
    "import pytestx\n",
  ] {
    assert!(
      layout(dir.path(), "other.py", quiet).is_empty(),
      "{quiet:?}"
    );
  }
  assert_eq!(
    layout(dir.path(), "conftest.py", "import pytest\n").len(),
    0
  );
  assert_eq!(
    layout(dir.path(), "__init__.py", "def test_a():\n").len(),
    0
  );
}

#[test]
fn a_test_file_needs_a_regular_module_beside_it() {
  let dir = tempfile::tempdir().unwrap();
  let alone = layout(dir.path(), "test_a.py", "def test_a():\n");
  assert_eq!(
    alone,
    [format!(
      "{ALONE}test_a.py — colocate test_*.py next to the module it tests"
    )]
  );
  std::fs::write(dir.path().join("conftest.py"), "").unwrap();
  std::os::unix::fs::symlink("/etc/hosts", dir.path().join("linked.py")).unwrap();
  assert_eq!(layout(dir.path(), "test_a.py", "").len(), 1);
  std::fs::write(dir.path().join("a.py"), "").unwrap();
  assert_eq!(layout(dir.path(), "test_a.py", "").len(), 0);
  assert_eq!(layout(dir.path(), "a_test.py", "").len(), 0);
}
