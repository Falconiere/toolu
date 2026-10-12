//! File and function size, ports of 25-size-file and 30-size-fn. The function
//! counter is the awk brace-depth scan line for line: it strips single-line
//! string contents, starts at a `function`, a `const NAME = (…` or
//! `= function`, or a method head, and ends where the depth returns to zero.

use toolu_runtime::config::quality::ThresholdSource;
use toolu_state::detect::lines::{count_code_lines, has_unterminated_block};
use toolu_state::detect::project::{TsLinter, ts_linter};

use crate::file::{Pattern, TsFile};

fn hint(linter: Option<TsLinter>, source: ThresholdSource, max: u64) -> String {
  let why = match (linter, source) {
    (Some(linter), ThresholdSource::Native) => {
      format!(" ({} enforces this max-lines limit)", linter.name())
    }
    (Some(TsLinter::Biome), ThresholdSource::Default) => {
      format!(" (biome has no max-lines equivalent — gate uses the {max}-line default)")
    }
    (Some(linter), ThresholdSource::Default) => format!(
      " ({} is present but the gate's limit didn't come from its config (unparsed config form or a per-glob override) — gate uses the {max}-line default; align them)",
      linter.name()
    ),
    _ => String::new(),
  };
  format!("split into smaller modules{why}")
}

/// 25-size-file.
pub(crate) fn file_too_long(f: &TsFile<'_>) -> Option<String> {
  let max = f.limits.file_lines;
  let count = count_code_lines(&f.file.absolute).unwrap_or(0);
  if count <= max {
    return None;
  }
  let linter = ts_linter(f.at.env, f.at.cwd);
  let hint = hint(linter, f.limits.file_source, max);
  let approx = if has_unterminated_block(&f.file.absolute) {
    " (size approximated — an unterminated /* or a string containing /* may be affecting the count)"
  } else {
    ""
  };
  Some(format!(
    "TS file exceeds {max}-line limit: {} ({count} code lines, blanks/comments excluded){approx} — {hint}",
    f.path()
  ))
}

static DOUBLE: Pattern = Pattern::new(r#""[^"]*""#);
static SINGLE: Pattern = Pattern::new("'[^']*'");
static BACKTICK: Pattern = Pattern::new("`[^`]*`");

/// awk `strip`: escaped quotes, then "…", '…' and `…` contents on one line.
fn strip(line: &str) -> String {
  let text = line.replace("\\\"", "");
  let text = DOUBLE.remove_all(&text);
  let text = SINGLE.remove_all(&text);
  BACKTICK.remove_all(&text)
}

static FUNCTION_DECL: Pattern = Pattern::new(
  r"^[[:space:]]*(export[[:space:]]+)?(default[[:space:]]+)?(async[[:space:]]+)?function[ \t*]",
);
static CONST_FN: Pattern = Pattern::new(
  r"^[[:space:]]*(export[[:space:]]+)?(default[[:space:]]+)?const[[:space:]]+[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*=[[:space:]]*(async[[:space:]]+)?(\(|function[ \t(*])",
);
static METHOD_HEAD: Pattern = Pattern::new(
  r"^[[:space:]]+(public[[:space:]]+|private[[:space:]]+|protected[[:space:]]+|static[[:space:]]+|async[[:space:]]+|override[[:space:]]+|readonly[[:space:]]+|get[[:space:]]+|set[[:space:]]+|\*[[:space:]]*)*[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*(<[^(){}]*>)?[[:space:]]*\(",
);
static KEYWORD_HEAD: Pattern = Pattern::new(
  r"^[[:space:]]*(if|for|while|switch|catch|return|do|else|function|await|with|yield|throw|new|typeof|delete|void|in|of|case)[^A-Za-z0-9_$]",
);
static METHOD_BODY: Pattern = Pattern::new(r"\)([[:space:]]*:[^={]*)?[[:space:]]*\{[[:space:]]*$");
static ENDS_STATEMENT: Pattern = Pattern::new(r";[[:space:]]*$");

fn starts_function(line: &str) -> bool {
  let s = strip(line);
  if FUNCTION_DECL.is_match(&s) || CONST_FN.is_match(&s) {
    return true;
  }
  METHOD_HEAD.is_match(&s)
    && !KEYWORD_HEAD.is_match(&s)
    && !s.contains("=>")
    && !ENDS_STATEMENT.is_match(&s)
    && METHOD_BODY.is_match(&s)
}

struct Open {
  start: usize,
  depth: i64,
  opened: bool,
}

fn braces(text: &str, brace: char) -> i64 {
  i64::try_from(text.matches(brace).count()).unwrap_or(i64::MAX)
}

/// Whether any function runs past `max` lines, measured as the awk counter did.
fn any_long_function(lines: &[String], max: u64) -> bool {
  let mut open: Option<Open> = None;
  let mut long = false;
  for (at, line) in lines.iter().enumerate() {
    if open.is_none() && starts_function(line) {
      open = Some(Open {
        start: at,
        depth: 0,
        opened: false,
      });
    }
    let Some(current) = open.as_mut() else {
      continue;
    };
    let s = strip(line);
    let opens = braces(&s, '{');
    current.depth += opens - braces(&s, '}');
    if opens > 0 {
      current.opened = true;
    }
    if current.opened && current.depth <= 0 {
      long |= u64::try_from(at - current.start).is_ok_and(|span| span > max);
      open = None;
    } else if !current.opened && ENDS_STATEMENT.is_match(line) {
      open = None;
    }
  }
  long
}

/// 30-size-fn.
pub(crate) fn function_too_long(f: &TsFile<'_>) -> Option<String> {
  any_long_function(&f.lines, f.limits.fn_lines).then(|| {
    format!(
      "Function too long in {} (>{} lines) — simplify or split",
      f.path(),
      f.limits.fn_lines
    )
  })
}

#[cfg(test)]
#[path = "tests/size_rules_test.rs"]
mod tests;
