//! The edited Rust file as every rule reads it, and the grep/awk idioms the
//! rules were built from. Patterns are POSIX EREs: Rust's `[[:space:]]` is the
//! same ASCII class, `.` also matches a newline, and `\b` is the ASCII word
//! boundary JavaScript used.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;

/// An ERE compiled on first use; one that fails to compile matches nothing.
pub(crate) type Rx = LazyLock<Option<Regex>>;

/// `source` compiled for an [`Rx`].
pub(crate) fn rx(source: &str) -> Option<Regex> {
  Regex::new(&format!("(?s){}", source.replace(r"\b", r"(?-u:\b)"))).ok()
}

/// Whether `pattern` matches `text`.
pub(crate) fn hit(pattern: &Rx, text: &str) -> bool {
  pattern.as_ref().is_some_and(|regex| regex.is_match(text))
}

/// The limits and switches the rules check against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Limits {
  /// `lang.rust.maxFileLines`, 500 by default.
  pub(crate) file_lines: u64,
  /// `lang.rust.maxFnLines`, 50 by default.
  pub(crate) fn_lines: u64,
  /// `lang.rust.maxImplLines`, 200 by default.
  pub(crate) impl_lines: u64,
  /// `lang.rust.noMocks`, true by default.
  pub(crate) no_mocks: bool,
  /// `rust-unsafe-exemptions.txt`: crates whose paths may hold `unsafe`.
  pub(crate) unsafe_exemptions: Vec<String>,
}

impl Default for Limits {
  /// The limits of a project with no `lang.rust` settings and no exemptions.
  fn default() -> Limits {
    Limits {
      file_lines: 500,
      fn_lines: 50,
      impl_lines: 200,
      no_mocks: true,
      unsafe_exemptions: Vec::new(),
    }
  }
}

/// One edited file and what the rules need to check it.
pub(crate) struct RsFile<'a> {
  /// The edited file.
  pub(crate) file: &'a EditedFile,
  /// The path the path rules test: the hook's path, or `/` and a
  /// repository-relative path, so a top-level `src/` is still under `/src/`.
  pub(crate) scope: &'a str,
  /// The lines as awk reads them: no trailing empty line for a final newline.
  pub(crate) lines: Vec<&'a str>,
  /// The resolved limits.
  pub(crate) limits: &'a Limits,
  /// The environment clippy is detected in.
  pub(crate) env: &'a Env,
  /// The directory clippy is detected from.
  pub(crate) cwd: &'a Path,
}

impl<'a> RsFile<'a> {
  /// The edited path, as the hook named it.
  pub(crate) fn path(&self) -> &'a str {
    &self.file.path
  }

  /// bash `[[ "$FILE_PATH" == *<part>* ]]`, over [`RsFile::scope`].
  pub(crate) fn path_has(&self, part: &str) -> bool {
    self.scope.contains(part)
  }
}

/// awk records of `text`.
pub(crate) fn split_lines(text: &str) -> Vec<&str> {
  match text.strip_suffix('\n') {
    _ if text.is_empty() => Vec::new(),
    Some(body) => body.split('\n').collect(),
    None => text.split('\n').collect(),
  }
}

static TEST_FILE_NAME: Rx = LazyLock::new(|| rx(r"_tests?\.rs$"));

/// `case "$(basename "$FILE_PATH")" in *_test.rs|*_tests.rs)`.
pub(crate) fn test_file_name(path: &str) -> bool {
  hit(&TEST_FILE_NAME, path.rsplit('/').next().unwrap_or(path))
}

#[cfg(test)]
#[path = "tests/file_test.rs"]
mod tests;
