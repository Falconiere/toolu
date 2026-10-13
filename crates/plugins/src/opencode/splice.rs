//! The two edits that keep a user's comments on their own entries, ported byte
//! for byte from `tools/toolu-cli` `src/opencode/jsonc.ts` (`appendJsonc`,
//! `removeJsonc`): an element on its own line is added or removed with its
//! line. A comment is one token, so a comma or line break inside one never
//! counts. Anything else falls back to the CST edits in `jsonc`.

use jsonc_parser::common::Ranged as _;
use jsonc_parser::tokens::Token;
use jsonc_parser::{CollectOptions, parse_to_ast};
use jsonc_parser::{Scanner, ScannerOptions};

use crate::opencode::jsonc::{append_element, remove_element, strict};

/// An array's `[`…`]` and its elements, as byte ranges.
struct ArrayAt {
  /// The offset of `]`.
  close: usize,
  /// Each element's start and end.
  elements: Vec<(usize, usize)>,
}

fn array_at(text: &str, key: &str) -> Option<ArrayAt> {
  let parsed = parse_to_ast(text, &CollectOptions::default(), &strict()).ok()?;
  let object = parsed.value.as_ref()?.as_object()?;
  let array = object.get(key)?.value.as_array()?;
  let elements = array.elements.iter().map(|element| {
    let range = element.range();
    (range.start, range.end)
  });
  Some(ArrayAt {
    close: array.range.end.checked_sub(1)?,
    elements: elements.collect(),
  })
}

fn part(text: &str, from: usize, to: usize) -> &str {
  text.get(from..to).unwrap_or_default()
}

fn line_start(text: &str, offset: usize) -> usize {
  part(text, 0, offset).rfind('\n').map_or(0, |at| at + 1)
}

/// Whether only whitespace precedes `offset` on its line.
fn own_line(text: &str, offset: usize) -> bool {
  part(text, line_start(text, offset), offset)
    .trim()
    .is_empty()
}

/// The first comma and every line break of `text[from, to)` outside comments.
struct Gap {
  comma: Option<usize>,
  breaks: Vec<(usize, usize)>,
}

fn gap(text: &str, from: usize, to: usize) -> Gap {
  let slice = part(text, from, to);
  let mut scanner = Scanner::new(slice, &ScannerOptions::default());
  let mut comma = None;
  let mut comments = Vec::new();
  while let Ok(Some(token)) = scanner.scan() {
    let span = (scanner.token_start(), scanner.token_end());
    if let Token::CommentLine(_) | Token::CommentBlock(_) = token {
      comments.push(span);
    } else if let Token::Comma = token
      && comma.is_none()
    {
      comma = Some(from + span.0);
    }
  }
  let outside = |at: usize| {
    !comments
      .iter()
      .any(|(start, end)| *start <= at && at < *end)
  };
  let breaks = slice
    .match_indices('\n')
    .map(|(at, _)| at)
    .filter(|at| outside(*at))
    .map(|at| {
      let crlf = at > 0 && slice.as_bytes().get(at - 1) == Some(&b'\r');
      let start = if crlf { at - 1 } else { at };
      (from + start, from + at + 1)
    })
    .collect();
  Gap { comma, breaks }
}

fn eol(text: &str) -> &'static str {
  if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Appends `spec` to the `key` array on a line of its own, after any comment
/// trailing the last element.
pub(crate) fn append(text: &str, key: &str, spec: &str) -> String {
  let Some(array) = array_at(text, key) else {
    return append_element(text, key, spec);
  };
  let Some(&(last_start, last_end)) = array.elements.last() else {
    return append_element(text, key, spec);
  };
  if !own_line(text, last_start) {
    return append_element(text, key, spec);
  }
  let after = gap(text, last_end, array.close);
  // Before the last line break ahead of `]`, or before `]` when none precedes it.
  let at = after.breaks.last().map_or(array.close, |(start, _)| *start);
  let trailing = after.comma.is_some();
  let indent = part(text, line_start(text, last_start), last_start);
  let quoted = serde_json::to_string(spec).unwrap_or_default();
  let entry = format!(
    "{}{indent}{quoted}{}",
    eol(text),
    if trailing { "," } else { "" }
  );
  let head = if trailing {
    part(text, 0, at).to_owned()
  } else {
    format!("{},{}", part(text, 0, last_end), part(text, last_end, at))
  };
  format!("{head}{entry}{}", part(text, at, text.len()))
}

/// Removes element `index` of the `key` array; on its own line it goes with its
/// line and trailing comment, and the neighbours keep theirs.
pub(crate) fn remove(text: &str, key: &str, index: usize) -> String {
  let Some(array) = array_at(text, key) else {
    return remove_element(text, key, index);
  };
  let Some(&(start, end)) = array.elements.get(index) else {
    return remove_element(text, key, index);
  };
  if !own_line(text, start) {
    return remove_element(text, key, index);
  }
  let next = array.elements.get(index + 1);
  let limit = next.map_or(array.close, |(next_start, _)| *next_start);
  let comma = gap(text, end, limit).comma;
  let line_end = gap(text, comma.unwrap_or(end), limit)
    .breaks
    .first()
    .copied();
  if line_end.is_none() && next.is_some() {
    return remove_element(text, key, index);
  }
  let resume = line_end.map_or(limit, |(_, after)| after);
  let body = format!(
    "{}{}",
    part(text, 0, line_start(text, start)),
    part(text, resume, text.len())
  );
  // The new last element must not keep a comma the removed one followed.
  let previous = index
    .checked_sub(1)
    .and_then(|before| array.elements.get(before));
  let Some(&(_, previous_end)) = previous.filter(|_| comma.is_none() && next.is_none()) else {
    return body;
  };
  match gap(text, previous_end, start).comma {
    Some(before) => format!(
      "{}{}",
      part(&body, 0, before),
      part(&body, before + 1, body.len())
    ),
    None => body,
  }
}

#[cfg(test)]
#[path = "tests/splice_test.rs"]
mod tests;
