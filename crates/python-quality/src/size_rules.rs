//! The size rules: file length in code lines (10-size-file) and the
//! indent-based function span (50-size-fn), ported from their awk programs.

use crate::file::{Ere, PyFile, ere, matches};

static DEF: Ere = Ere::new(|| ere(r"^[ \t]*(async[ \t]+)?def[ \t]"));
static NAME: Ere = Ere::new(|| ere(r"def[ \t]+[A-Za-z_][A-Za-z0-9_]*"));

/// `count_python_code_lines`'s code line: its first byte that is not a space or a
/// tab exists and is not `#`, so a `\r` alone is code, as awk reads it.
fn is_code(line: &str) -> bool {
  line
    .trim_start_matches([' ', '\t'])
    .chars()
    .next()
    .is_some_and(|first| first != '#')
}

/// 10-size-file: non-blank, non-`#` lines over `lang.python.maxFileLines`, counted
/// in the text already read, so the file is never read twice.
pub(crate) fn file_too_long(f: &PyFile<'_>) -> Option<String> {
  let max = f.limits.file_lines;
  let code = f.lines.iter().filter(|line| is_code(line)).count();
  let count = u64::try_from(code).unwrap_or(u64::MAX);
  (count > max).then(|| {
    format!(
      "File exceeds {max}-line limit: {} ({count} code lines, blanks/comments excluded) — split \
       into submodules. Override via lang.python.maxFileLines.",
      f.file.path
    )
  })
}

/// awk `lead`: leading spaces and tabs.
fn lead(line: &str) -> usize {
  line.len() - line.trim_start_matches([' ', '\t']).len()
}

/// The defined name of a `def` line with its indent removed.
fn def_name(trimmed: &str) -> String {
  let found = NAME
    .as_ref()
    .and_then(|regex| regex.find(trimmed))
    .map_or("", |hit| hit.as_str());
  found
    .strip_prefix("def")
    .unwrap_or(found)
    .trim_start_matches([' ', '\t'])
    .to_owned()
}

/// One open function span.
struct Span {
  name: String,
  start: usize,
  indent: usize,
  count: u64,
}

impl Span {
  fn report(&self) -> String {
    format!("{}:{} ({} lines)", self.name, self.start, self.count)
  }
}

/// 50-size-fn: a `def` opens a span at its own indent, closed by the next
/// non-blank line indented no deeper. Only code lines count, the def line
/// included; a def nested in an open span belongs to it.
pub(crate) fn function_too_long(f: &PyFile<'_>) -> Option<String> {
  let max = f.limits.fn_lines;
  let mut long = Vec::new();
  let mut open: Option<Span> = None;
  for (index, raw) in f.lines.iter().enumerate() {
    let trimmed = raw.trim_start_matches([' ', '\t']);
    let blank = trimmed.is_empty();
    if !blank && open.as_ref().is_some_and(|span| lead(raw) <= span.indent) {
      long.extend(
        open
          .take()
          .filter(|span| span.count > max)
          .map(|span| span.report()),
      );
    }
    if open.is_none() && matches(&DEF, raw) {
      open = Some(Span {
        name: def_name(trimmed),
        start: index + 1,
        indent: lead(raw),
        count: 0,
      });
    }
    if let Some(span) = open.as_mut()
      && !blank
      && !trimmed.starts_with('#')
    {
      span.count += 1;
    }
  }
  long.extend(
    open
      .filter(|span| span.count > max)
      .map(|span| span.report()),
  );
  (!long.is_empty()).then(|| {
    format!(
      "Function too long in {} (>{max} lines) — extract helpers.\n{}",
      f.file.path,
      long.join("\n")
    )
  })
}

#[cfg(test)]
#[path = "tests/size_rules_test.rs"]
mod tests;
