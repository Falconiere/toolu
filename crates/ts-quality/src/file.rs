//! The edited TypeScript file as every rule reads it, and the grep idioms the
//! rules were built from: lines as grep sees them, `grep -n` excerpts, `head -N`.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use toolu_engine::quality::EditedFile;
use toolu_runtime::config::quality::ThresholdSource;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::detect::project::NodePackageManager;

/// A POSIX ERE compiled on first use. `.` also matches a newline, and `\b` is
/// the ASCII word boundary, as in the JavaScript the rules were ported from. A
/// pattern that fails to compile matches nothing.
pub(crate) struct Pattern {
  source: &'static str,
  cell: OnceLock<Option<Regex>>,
}

impl Pattern {
  /// The pattern for `source`.
  pub(crate) const fn new(source: &'static str) -> Pattern {
    Pattern {
      source,
      cell: OnceLock::new(),
    }
  }

  fn regex(&self) -> Option<&Regex> {
    self
      .cell
      .get_or_init(|| compile(self.source))
      .as_ref()
  }

  /// Whether `text` matches.
  pub(crate) fn is_match(&self, text: &str) -> bool {
    self.regex().is_some_and(|regex| regex.is_match(text))
  }

  /// The first capture group of the first match.
  pub(crate) fn first_group<'t>(&self, text: &'t str) -> Option<&'t str> {
    let captures = self.regex()?.captures(text)?;
    captures.get(1).map(|group| group.as_str())
  }

  /// `text` with every match removed.
  pub(crate) fn remove_all(&self, text: &str) -> String {
    match self.regex() {
      Some(regex) => regex.replace_all(text, "").into_owned(),
      None => text.to_owned(),
    }
  }
}

/// `source` as a Rust regex with JavaScript's `s` flag and ASCII `\b`.
pub(crate) fn compile(source: &str) -> Option<Regex> {
  Regex::new(&format!("(?s){}", source.replace(r"\b", r"(?-u:\b)"))).ok()
}

/// The limits the size and mock rules check against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
  /// `lang.ts.maxFileLines`, the linter's `max-lines`, or 300.
  pub(crate) file_lines: u64,
  /// Where `file_lines` came from.
  pub(crate) file_source: ThresholdSource,
  /// `lang.ts.maxFnLines`.
  pub(crate) fn_lines: u64,
  /// `lang.ts.noMocks`, true by default.
  pub(crate) no_mocks: bool,
}

/// Where the hook runs.
#[derive(Clone, Copy)]
pub(crate) struct Place<'a> {
  /// The hook's environment.
  pub(crate) env: &'a Env,
  /// The project root.
  pub(crate) project_root: &'a Path,
  /// The hook's working directory.
  pub(crate) cwd: &'a Path,
}

impl<'a> Place<'a> {
  /// The place of `ctx`; its working directory falls back to the project root.
  pub(crate) fn of(ctx: &RuleContext<'a>) -> Place<'a> {
    Place {
      env: ctx.env,
      project_root: ctx.project_root,
      cwd: ctx.cwd.unwrap_or(ctx.project_root),
    }
  }
}

/// One edited file and what the rules need to check it.
pub(crate) struct TsFile<'a> {
  /// The edited file.
  pub(crate) file: &'a EditedFile,
  /// The lines as grep reads them: no trailing empty line for a final newline.
  pub(crate) lines: Vec<String>,
  /// Where the hook runs.
  pub(crate) at: Place<'a>,
  /// The resolved limits.
  pub(crate) limits: Limits,
  /// The project's package manager.
  pub(crate) pm: NodePackageManager,
}

impl TsFile<'_> {
  /// The edited path, as the hook named it.
  pub(crate) fn path(&self) -> &str {
    &self.file.path
  }
}

/// `grep` / `awk` lines of `text`.
pub(crate) fn split_lines(text: &str) -> Vec<String> {
  if text.is_empty() {
    return Vec::new();
  }
  let body = text.strip_suffix('\n').unwrap_or(text);
  body.split('\n').map(str::to_owned).collect()
}

/// `grep -n PATTERN`: `N:line` for every matching line.
pub(crate) fn numbered(lines: &[String], pattern: &Pattern) -> Vec<String> {
  lines
    .iter()
    .enumerate()
    .filter(|(_, line)| pattern.is_match(line))
    .map(|(at, line)| format!("{}:{line}", at + 1))
    .collect()
}

/// `grep -c PATTERN`.
pub(crate) fn count_matches(lines: &[String], pattern: &Pattern) -> usize {
  lines.iter().filter(|line| pattern.is_match(line)).count()
}

/// `head -N`, joined as `$(...)` returns it: newline-separated, no trailing newline.
pub(crate) fn head(items: &[String], n: usize) -> String {
  items.iter().take(n).cloned().collect::<Vec<_>>().join("\n")
}

static COMMENT_ROW: Pattern = Pattern::new("^[0-9]+:[[:space:]]*//");

/// Over `grep -n` output, drop whole-line `//` comments.
pub(crate) fn without_comment_lines(rows: Vec<String>) -> Vec<String> {
  rows
    .into_iter()
    .filter(|row| !COMMENT_ROW.is_match(row))
    .collect()
}

/// `<header>\n<excerpt>`, or `None` when the excerpt is empty.
pub(crate) fn with_excerpt(header: &str, excerpt: &str) -> Option<String> {
  (!excerpt.is_empty()).then(|| format!("{header}\n{excerpt}"))
}

/// Whether `name`'s extension is one of `extensions`, case-sensitively.
pub(crate) fn has_extension(name: &str, extensions: &[&str]) -> bool {
  Path::new(name)
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| extensions.contains(&extension))
}

static TEST_PATH: Pattern = Pattern::new(r"\.(test|spec)\.(ts|tsx)$");

/// `\.(test|spec)\.(ts|tsx)$` on the path.
pub(crate) fn is_test_path(path: &str) -> bool {
  TEST_PATH.is_match(path)
}

#[cfg(test)]
#[path = "tests/file_test.rs"]
mod tests;
