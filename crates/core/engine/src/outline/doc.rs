//! The first doc sentence of an item: `///` lines, `/** */` blocks and Python docstrings.

use super::text::{Cut, cap};

const DOC_CHARS: usize = 100;

/// The text of the `///` lines and `/** */` blocks among `above`.
pub(super) fn above(above: &[&str]) -> Option<String> {
  let mut text = Vec::new();
  let mut in_block = false;
  for line in above.iter().map(|line| line.trim()) {
    let content = if let Some(rest) = line.strip_prefix("/**") {
      in_block = !rest.ends_with("*/");
      rest.trim_end_matches("*/")
    } else if in_block {
      in_block = !line.ends_with("*/");
      line.trim_end_matches("*/").trim_start_matches('*')
    } else if let Some(rest) = line.strip_prefix("///") {
      rest
    } else {
      continue;
    };
    text.push(content.trim());
  }
  paragraph(&text)
}

/// The docstring opening a Python body after the signature's `:` at `cut`.
pub(super) fn docstring(lines: &[&str], cut: Cut, end: usize) -> Option<String> {
  let (index, pos) = cut?;
  let mut body = lines.get(index)?.trim().get(pos + 1..)?.to_owned();
  for line in lines.get(index + 1..=end).unwrap_or_default() {
    body.push('\n');
    body.push_str(line);
  }
  let body = body.trim_start().trim_start_matches(['r', 'u', 'R', 'U']);
  let quote = ["\"\"\"", "'''", "\"", "'"]
    .into_iter()
    .find(|quote| body.starts_with(quote))?;
  let inner = body.get(quote.len()..)?;
  let inner = inner
    .find(quote)
    .and_then(|close| inner.get(..close))
    .unwrap_or(inner);
  paragraph(&inner.lines().map(str::trim).collect::<Vec<_>>())
}

/// The first paragraph of `text`, without `@` tag lines.
fn paragraph(text: &[&str]) -> Option<String> {
  let words: Vec<&str> = text
    .iter()
    .skip_while(|line| line.is_empty())
    .take_while(|line| !line.is_empty() && !line.starts_with('@'))
    .copied()
    .collect();
  (!words.is_empty()).then(|| words.join(" "))
}

pub(super) fn sentence(text: &str) -> Option<String> {
  let first = text
    .find(". ")
    .and_then(|end| text.get(..=end))
    .unwrap_or(text);
  let first = first.trim();
  (!first.is_empty()).then(|| cap(first, DOC_CHARS))
}

#[cfg(test)]
#[path = "tests/doc_test.rs"]
mod tests;
