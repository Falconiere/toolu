//! The edited Python file as every rule reads it, and the grep/awk idioms the
//! rules were built from. awk splits records on `\n` only, so a CRLF line keeps
//! its `\r`: `[ \t]` does not match it, POSIX `[[:space:]]` does, and the Rust
//! regex class `[[:space:]]` is the same ASCII set.

use std::sync::LazyLock;

use regex::Regex;
use toolu_engine::quality::EditedFile;

/// A regex compiled on first use, with JavaScript's `s` flag (`.` also matches
/// a newline). A pattern that fails to compile matches nothing.
pub(crate) type Ere = LazyLock<Option<Regex>>;

/// `source` with the `s` flag, for an [`Ere`].
pub(crate) fn ere(source: &str) -> Option<Regex> {
  Regex::new(&format!("(?s){source}")).ok()
}

/// Whether the compiled `pattern` matches `text`.
pub(crate) fn matches(pattern: &Ere, text: &str) -> bool {
  pattern.as_ref().is_some_and(|regex| regex.is_match(text))
}

/// The limits the rules check against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
  /// `lang.python.maxFileLines`, 400 by default.
  pub(crate) file_lines: u64,
  /// `lang.python.maxFnLines`, 50 by default.
  pub(crate) fn_lines: u64,
  /// `lang.python.noMocks`, true by default.
  pub(crate) no_mocks: bool,
}

impl Default for Limits {
  /// The limits of a project with no `lang.python` settings.
  fn default() -> Limits {
    Limits {
      file_lines: 400,
      fn_lines: 50,
      no_mocks: true,
    }
  }
}

/// One edited file and what the rules need to check it.
pub(crate) struct PyFile<'a> {
  /// The edited file.
  pub(crate) file: &'a EditedFile,
  /// The lines as awk reads them: no trailing empty line for a final newline.
  pub(crate) lines: Vec<&'a str>,
  /// The resolved limits.
  pub(crate) limits: Limits,
}

impl<'a> PyFile<'a> {
  /// `file` with its `text` split as awk reads it.
  pub(crate) fn new(file: &'a EditedFile, text: &'a str, limits: Limits) -> PyFile<'a> {
    PyFile {
      file,
      lines: split_lines(text),
      limits,
    }
  }
}

/// `grep` / `awk` lines of `text`.
pub(crate) fn split_lines(text: &str) -> Vec<&str> {
  if text.is_empty() {
    return Vec::new();
  }
  let mut lines: Vec<&str> = text.split('\n').collect();
  if text.ends_with('\n') {
    lines.pop();
  }
  lines
}

/// `head -N`, joined as `$(...)` returns it: newline-separated, no trailing newline.
pub(crate) fn head(items: &[String], n: usize) -> String {
  items
    .iter()
    .take(n)
    .map(String::as_str)
    .collect::<Vec<_>>()
    .join("\n")
}

/// `*.py` as a shell glob reads it: the exact, case-sensitive suffix.
pub(crate) fn ends_with_py(name: &str) -> bool {
  name
    .strip_suffix("py")
    .is_some_and(|stem| stem.ends_with('.'))
}

/// `case "$base" in test_*.py|*_test.py)`: the colocated test naming convention.
pub(crate) fn is_python_test_name(base: &str) -> bool {
  (base.starts_with("test_") && ends_with_py(base)) || base.ends_with("_test.py")
}

/// The last component of `path`, as `basename` prints it.
pub(crate) fn base_name(path: &str) -> &str {
  let trimmed = path.trim_end_matches('/');
  trimmed.rsplit('/').next().unwrap_or(trimmed)
}

#[cfg(test)]
#[path = "tests/file_test.rs"]
mod tests;
