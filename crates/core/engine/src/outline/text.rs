//! Item fields from the file text: the range over doc and attribute lines, the
//! one-line signature, the first doc sentence and visibility.

use super::Item;
use super::parse::Match;
use super::rules::Language;

const SIGNATURE_CHARS: usize = 160;
const DOC_CHARS: usize = 100;

/// Where a signature stopped: the line index and the byte offset of its body opener.
type Cut = Option<(usize, usize)>;

/// The item for one match over `lines`; `depth` is set by the caller.
pub(super) fn item(lines: &[&str], found: &Match, language: Language) -> Item {
  let end = found.end.min(lines.len().saturating_sub(1));
  let start = found.start.min(end);
  let top = extend(lines, start, language);
  let (signature, cut) = signature(lines, start, end, language);
  let doc = if language == Language::Python {
    docstring(lines, cut, end)
  } else {
    doc_above(lines.get(top..start).unwrap_or_default())
  };
  let public = match language {
    Language::Rust => signature.starts_with("pub ") || signature.starts_with("pub("),
    Language::Script => script_public(&signature),
    Language::Python => !found.name.starts_with('_'),
  };
  Item {
    name: found.name.clone(),
    signature: cap(&signature, SIGNATURE_CHARS),
    doc: doc.as_deref().and_then(sentence),
    start: top + 1,
    end: end + 1,
    depth: 0,
    public,
  }
}

fn script_public(signature: &str) -> bool {
  const DECLARATIONS: &[&str] = &[
    "function",
    "class",
    "abstract",
    "interface",
    "type",
    "enum",
    "namespace",
    "module",
    "const",
    "let",
    "var",
    "declare",
  ];
  let first = signature.split_whitespace().next().unwrap_or_default();
  if first == "export" {
    return true;
  }
  if first == "private" || first == "protected" || first.starts_with('#') {
    return false;
  }
  let declaration = signature.starts_with("async function") || DECLARATIONS.contains(&first);
  !declaration
}

fn decorator(language: Language, line: &str) -> bool {
  language != Language::Rust && line.starts_with('@')
}

/// The first line of the doc, attribute and decorator lines directly above `start`.
fn extend(lines: &[&str], start: usize, language: Language) -> usize {
  let mut top = start;
  while let Some(index) = top.checked_sub(1) {
    let above = lines.get(index).map_or("", |line| line.trim());
    let next = if above.starts_with("///") || above.starts_with("#[") || decorator(language, above)
    {
      Some(index)
    } else if above.ends_with("*/") {
      block_opener(lines, index)
    } else if language == Language::Rust && above.ends_with(']') {
      opener(lines, index, "#[")
    } else if language != Language::Rust && above.ends_with(')') {
      opener(lines, index, "@")
    } else {
      None
    };
    let Some(next) = next else { break };
    top = next;
  }
  top
}

/// The `/**` line opening the doc comment that ends on `from`, walking only
/// over its `*` continuation lines.
fn block_opener(lines: &[&str], from: usize) -> Option<usize> {
  for index in (0..=from).rev() {
    let line = lines.get(index)?.trim();
    if line.starts_with("/**") {
      return Some(index);
    }
    if !line.starts_with('*') {
      return None;
    }
  }
  None
}

/// The line where the brackets that close on `from` open, when it starts with
/// `prefix`: the first line of a multi-line attribute or decorator.
fn opener(lines: &[&str], from: usize, prefix: &str) -> Option<usize> {
  let mut unclosed = 0_i64;
  for index in (0..=from).rev() {
    let line = lines.get(index)?.trim();
    for ch in line.chars() {
      match ch {
        ')' | ']' | '}' => unclosed += 1,
        '(' | '[' | '{' => unclosed -= 1,
        _ => {}
      }
    }
    if unclosed <= 0 {
      return line.starts_with(prefix).then_some(index);
    }
    if line.is_empty() {
      return None;
    }
  }
  None
}

/// The declaration from `start` up to its body opener, whitespace collapsed.
fn signature(lines: &[&str], start: usize, end: usize, language: Language) -> (String, Cut) {
  let mut out = String::new();
  let mut state = Scan::default();
  let mut leading = true;
  for index in start..=end {
    let line = lines.get(index).map_or("", |line| line.trim());
    if leading && decorator(language, line) {
      continue;
    }
    leading = false;
    if !line.is_empty() && !out.is_empty() && !out.ends_with(['(', '[']) {
      out.push(' ');
    }
    if let Some(pos) = scan_line(line, language, &mut state, &mut out) {
      return (finish(&out), Some((index, pos)));
    }
  }
  (finish(&out), None)
}

/// `out` with single spaces, no space inside brackets and no trailing comma.
fn finish(out: &str) -> String {
  let single = out.split_whitespace().collect::<Vec<_>>().join(" ");
  let tight = single.replace("( ", "(").replace("[ ", "[");
  tight.trim_end_matches(',').trim_end().to_owned()
}

/// Bracket depth and the open string literal while reading a signature.
#[derive(Default)]
struct Scan {
  depth: usize,
  quote: Option<char>,
  escaped: bool,
}

impl Scan {
  /// Whether `ch` lies inside, or closes, an open string literal.
  fn in_string(&mut self, ch: char) -> bool {
    let Some(quote) = self.quote else {
      return false;
    };
    if self.escaped {
      self.escaped = false;
    } else if ch == '\\' {
      self.escaped = true;
    } else if ch == quote {
      self.quote = None;
    }
    true
  }
}

/// Append `line` to `out` until its body opener, returning the opener's byte offset.
fn scan_line(line: &str, language: Language, state: &mut Scan, out: &mut String) -> Option<usize> {
  let python = language == Language::Python;
  for (pos, ch) in line.char_indices() {
    if state.in_string(ch) {
      out.push(ch);
      continue;
    }
    let rest = line.get(pos..).unwrap_or_default();
    if rest.starts_with("//") && !python || rest.starts_with('#') && python {
      return None;
    }
    let opener = if python {
      ch == ':'
    } else {
      ch == '{' || ch == ';'
    };
    if state.depth == 0 && opener {
      return Some(pos);
    }
    match ch {
      '"' | '`' => state.quote = Some(ch),
      '\'' if language != Language::Rust => state.quote = Some(ch),
      '(' | '[' | '{' => state.depth += 1,
      ')' | ']' | '}' => {
        state.depth = state.depth.saturating_sub(1);
        let kept = out.trim_end().trim_end_matches(',').trim_end().len();
        out.truncate(kept);
      }
      _ => {}
    }
    out.push(ch);
  }
  None
}

/// The text of the `///` lines and `/** */` blocks among `above`.
fn doc_above(above: &[&str]) -> Option<String> {
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
fn docstring(lines: &[&str], cut: Cut, end: usize) -> Option<String> {
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

fn sentence(text: &str) -> Option<String> {
  let first = text
    .find(". ")
    .and_then(|end| text.get(..=end))
    .unwrap_or(text);
  let first = first.trim();
  (!first.is_empty()).then(|| cap(first, DOC_CHARS))
}

fn cap(text: &str, limit: usize) -> String {
  if text.chars().count() <= limit {
    return text.to_owned();
  }
  let mut short: String = text.chars().take(limit - 3).collect();
  short.push_str("...");
  short
}

#[cfg(test)]
#[path = "tests/text_test.rs"]
mod tests;
