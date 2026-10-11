//! Compiled regular expressions without a panicking path: a pattern that fails
//! to compile matches nothing, and the unit tests compile every one.

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// A lazily compiled pattern.
pub(crate) type Pattern = LazyLock<Option<Regex>>;

/// Declare `static NAME: Pattern` for a regular expression.
macro_rules! pattern {
  ($name:ident, $source:expr) => {
    static $name: $crate::pattern::Pattern =
      std::sync::LazyLock::new(|| regex::Regex::new($source).ok());
  };
}
pub(crate) use pattern;

/// Whether `pattern` matches anywhere in `text`.
pub(crate) fn is(pattern: &Pattern, text: &str) -> bool {
  pattern.as_ref().is_some_and(|re| re.is_match(text))
}

/// The captures of the first match of `pattern` in `text`.
pub(crate) fn captures<'t>(pattern: &Pattern, text: &'t str) -> Option<Captures<'t>> {
  pattern.as_ref().and_then(|re| re.captures(text))
}

/// Capture group `group` of the first match, as an owned string.
pub(crate) fn group(pattern: &Pattern, text: &str, group: usize) -> Option<String> {
  captures(pattern, text).and_then(|caps| caps.get(group).map(|found| found.as_str().to_owned()))
}

#[cfg(test)]
#[path = "tests/pattern_test.rs"]
mod tests;
