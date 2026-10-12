//! Test placement (20-tests): a test-bearing file must follow the `test_*.py` /
//! `*_test.py` convention, and a `test_*.py` needs a non-test module beside it.
//! `conftest.py` and `__init__.py` are exempt from both.

use std::fs;
use std::path::Path;

use crate::file::{Ere, PyFile, base_name, ends_with_py, ere, is_python_test_name, matches};

static TEST_DEF: Ere =
  Ere::new(|| ere(r"^(async[[:space:]]+)?def[[:space:]]+test_[A-Za-z0-9_]*[[:space:]]*\("));
static TEST_IMPORT: Ere = Ere::new(|| ere(r"^(import|from)[[:space:]]+(pytest|unittest)"));

/// grep's `\b` after the module name: the next character is not a word character.
fn imports_a_test_framework(line: &str) -> bool {
  let Some(found) = TEST_IMPORT.as_ref().and_then(|regex| regex.find(line)) else {
    return false;
  };
  !line
    .get(found.end()..)
    .unwrap_or_default()
    .chars()
    .next()
    .is_some_and(|next| next.is_ascii_alphanumeric() || next == '_')
}

fn is_module_name(name: &str) -> bool {
  ends_with_py(name) && !is_python_test_name(name) && name != "conftest.py" && name != "__init__.py"
}

/// `find DIR -maxdepth 1 -type f -name '*.py' ! -name <tests…>`: a regular
/// module file, symlinks excluded.
fn has_module_sibling(dir: &Path) -> bool {
  let Ok(entries) = fs::read_dir(dir) else {
    return false;
  };
  entries.flatten().any(|entry| {
    is_module_name(&entry.file_name().to_string_lossy())
      && fs::symlink_metadata(entry.path()).is_ok_and(|meta| meta.is_file())
  })
}

/// The placement violations of `f`.
pub(crate) fn test_layout(f: &PyFile<'_>) -> Vec<String> {
  let path = &f.file.path;
  let base = base_name(path);
  if base == "conftest.py" || base == "__init__.py" {
    return Vec::new();
  }
  let mut errors = Vec::new();
  let marker = f
    .lines
    .iter()
    .any(|line| matches(&TEST_DEF, line) || imports_a_test_framework(line));
  if marker && !is_python_test_name(base) {
    errors.push(format!(
      "Test-bearing file not named test_*.py or *_test.py: {path} — rename to follow the \
       colocated test convention"
    ));
  }
  let dir = f.file.absolute.parent().unwrap_or(&f.file.absolute);
  if base.starts_with("test_") && ends_with_py(base) && !has_module_sibling(dir) {
    errors.push(format!(
      "Test not co-located with a module: {path} — colocate test_*.py next to the module it tests"
    ));
  }
  errors
}

#[cfg(test)]
#[path = "tests/layout_rules_test.rs"]
mod tests;
