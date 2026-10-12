//! The docs advisory (90-docs): a column-0, non-underscore `def` or `class`
//! should open with a docstring. The signature is followed to the line ending in
//! `:`, then the first non-blank line must open a `"""` or `'''` string (one
//! `r`/`R`/`f`/`F` prefix allowed). A line consumed as that body is not itself
//! checked for a definition, as in the awk program.

use crate::file::{Ere, PyFile, ere, head, matches};

static DEFINITION: Ere =
  Ere::new(|| ere(r"^(async[ \t]+def|def|class)[ \t]+[A-Za-z_][A-Za-z0-9_]*"));
static KEYWORD: Ere = Ere::new(|| ere(r"^(async[ \t]+def|def|class)[ \t]+"));
static IDENTIFIER: Ere = Ere::new(|| ere(r"^[A-Za-z_][A-Za-z0-9_]*"));
static COMMENT: Ere = Ere::new(|| ere(r"[ \t]#.*$"));

const BLANKS: [char; 2] = [' ', '\t'];

fn is_blank(line: &str) -> bool {
  line.trim_matches(BLANKS).is_empty()
}

fn ends_with_colon(line: &str) -> bool {
  let code = COMMENT
    .as_ref()
    .map_or_else(|| line.into(), |regex| regex.replacen(line, 1, ""));
  code.trim_end_matches(BLANKS).ends_with(':')
}

fn opens_docstring(line: &str) -> bool {
  let text = line.trim_start_matches(BLANKS);
  let body = text.strip_prefix(['r', 'R', 'f', 'F']).unwrap_or(text);
  body.starts_with("\"\"\"") || body.starts_with("'''")
}

fn definition_name(line: &str) -> &str {
  let rest = KEYWORD
    .as_ref()
    .and_then(|regex| regex.find(line))
    .map_or(line, |found| line.get(found.end()..).unwrap_or_default());
  IDENTIFIER
    .as_ref()
    .and_then(|regex| regex.find(rest))
    .map_or("", |found| found.as_str())
}

enum State {
  Scan,
  Signature,
  Body,
}

/// The advisory text, or "" when every public top-level definition has a docstring.
pub(crate) fn docs(f: &PyFile<'_>) -> String {
  let mut missing = Vec::new();
  let mut state = State::Scan;
  let mut at = (0, "");
  for (index, line) in f.lines.iter().enumerate() {
    match state {
      State::Body => {
        if is_blank(line) {
          continue;
        }
        if !opens_docstring(line) {
          missing.push(format!("{}: {}", at.0, at.1));
        }
        state = State::Scan;
      }
      State::Signature => {
        if ends_with_colon(line) {
          state = State::Body;
        }
      }
      State::Scan => {
        if !matches(&DEFINITION, line) {
          continue;
        }
        let name = definition_name(line);
        if name.starts_with('_') {
          continue;
        }
        at = (index + 1, name);
        state = if ends_with_colon(line) {
          State::Body
        } else {
          State::Signature
        };
      }
    }
  }
  if missing.is_empty() {
    return String::new();
  }
  format!(
    "Public def/class missing a docstring in {} — add a concise one:\n{}",
    f.file.path,
    head(&missing, 3)
  )
}

#[cfg(test)]
#[path = "tests/docs_test.rs"]
mod tests;
