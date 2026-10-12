//! File, function and impl size, ports of 10-size-file, 50-size-fn and
//! 55-size-impl. The fn and impl counters are the awk brace-depth scan line for
//! line: single-line string contents and `'{'`/`'}'` literals are stripped, the
//! range starts at the head and ends where the depth returns to zero.

use std::sync::LazyLock;

use toolu_state::detect::lines::{count_code_lines, has_unterminated_block};
use toolu_state::detect::project::detect_clippy;

use crate::file::{RsFile, Rx, hit, rx};

/// 10-size-file.
pub(crate) fn file_too_long(f: &RsFile<'_>) -> Option<String> {
  let max = f.limits.file_lines;
  let count = count_code_lines(&f.file.absolute).unwrap_or(0);
  if count <= max {
    return None;
  }
  let hint = if detect_clippy(f.env, f.cwd) {
    "split into submodules (clippy enforces complexity here)"
  } else {
    "split into submodules"
  };
  let approx = if has_unterminated_block(&f.file.absolute) {
    " (size approximated — an unterminated /* or a string containing /* may be affecting the count)"
  } else {
    ""
  };
  Some(format!(
    "File exceeds {max}-line limit: {} ({count} code lines, blanks/comments excluded){approx} — {hint}",
    f.path()
  ))
}

static QUOTED: Rx = LazyLock::new(|| rx(r#""[^"]*""#));
static BRACE_CHAR: Rx = LazyLock::new(|| rx(r"'[{}]'"));

/// awk: escaped quotes, then single-line "…" contents, then brace char literals.
fn strip(line: &str) -> String {
  let unescaped = line.replace("\\\"", "");
  let Some((quoted, brace_char)) = QUOTED.as_ref().zip(BRACE_CHAR.as_ref()) else {
    return unescaped;
  };
  let unquoted = quoted.replace_all(&unescaped, "");
  brace_char.replace_all(&unquoted, "").into_owned()
}

static FN_HEAD: Rx = LazyLock::new(|| {
  rx(
    r#"^[[:space:]]*(pub(\([^)]+\))?[[:space:]]+)?((async|const|unsafe|extern)([[:space:]]+"[^"]*")?[[:space:]]+)*fn "#,
  )
});
static IMPL_HEAD: Rx =
  LazyLock::new(|| rx(r"^[[:space:]]*(unsafe[[:space:]]+)?impl[ \t\n\v\f\r<]"));
static ENDS_STATEMENT: Rx = LazyLock::new(|| rx(r";[[:space:]]*$"));

/// The range being measured: its first line, brace depth, and whether a `{` opened it.
type Range = (usize, i64, bool);

fn net_depth(stripped: &str) -> (i64, bool) {
  let opens = stripped.matches('{').count();
  let closes = stripped.matches('}').count();
  let delta = i64::try_from(opens).unwrap_or(i64::MAX) - i64::try_from(closes).unwrap_or(i64::MAX);
  (delta, opens > 0)
}

/// Whether any range opened by `head` runs past `max` lines. With
/// `release_on_semicolon`, a head whose line ends in `;` before any `{` is a
/// body-less declaration.
fn any_long_range(lines: &[&str], head: &Rx, max: u64, release_on_semicolon: bool) -> bool {
  let mut range: Option<Range> = None;
  let mut long = false;
  for (row, line) in lines.iter().enumerate() {
    let (start, depth, opened) = match range {
      Some(open) => open,
      None if hit(head, line) => (row, 0, false),
      None => continue,
    };
    let (delta, opens) = net_depth(&strip(line));
    let (depth, opened) = (depth + delta, opened || opens);
    range = if opened && depth <= 0 {
      long |= u64::try_from(row - start).is_ok_and(|span| span > max);
      None
    } else if release_on_semicolon && !opened && hit(&ENDS_STATEMENT, line) {
      None
    } else {
      Some((start, depth, opened))
    };
  }
  long
}

/// 50-size-fn; test files are exempt.
pub(crate) fn function_too_long(f: &RsFile<'_>, is_test: bool) -> Option<String> {
  let max = f.limits.fn_lines;
  (!is_test && any_long_range(&f.lines, &FN_HEAD, max, true)).then(|| {
    format!(
      "Function too long in {} (>{max} lines) — extract helpers.",
      f.path()
    )
  })
}

/// 55-size-impl; test files are exempt.
pub(crate) fn impl_too_long(f: &RsFile<'_>, is_test: bool) -> Option<String> {
  let max = f.limits.impl_lines;
  (!is_test && any_long_range(&f.lines, &IMPL_HEAD, max, false)).then(|| {
    format!(
      "Impl block too large in {} (>{max} lines) — split into trait impls or modules.",
      f.path()
    )
  })
}

#[cfg(test)]
#[path = "tests/size_rules_test.rs"]
mod tests;
