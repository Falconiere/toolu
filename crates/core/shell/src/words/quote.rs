//! Quote removal, as bash applies it to a word's literal text: backslash escapes
//! outside quotes and inside double quotes, ANSI-C `$'…'` decoding (unbash's
//! `decodeAnsiCQuoted`), and the pathname-pattern test.

/// An unquoted literal: `\x` is `x`, and a backslash-newline disappears.
pub(crate) fn unquoted(raw: &str) -> String {
  let mut value = String::with_capacity(raw.len());
  let mut chars = raw.chars();
  while let Some(c) = chars.next() {
    match (c, c == '\\') {
      (_, true) => match chars.next() {
        Some('\n') => {}
        Some(escaped) => value.push(escaped),
        None => value.push('\\'),
      },
      (c, false) => value.push(c),
    }
  }
  value
}

/// Inside double quotes only `\$`, `` \` ``, `\"`, `\\` and a backslash-newline escape.
pub(crate) fn double_quoted(raw: &str) -> String {
  let mut value = String::with_capacity(raw.len());
  let mut chars = raw.chars().peekable();
  while let Some(c) = chars.next() {
    if c != '\\' {
      value.push(c);
      continue;
    }
    match chars.peek().copied() {
      Some('\n') => {
        chars.next();
      }
      Some(escaped @ ('$' | '`' | '"' | '\\')) => {
        chars.next();
        value.push(escaped);
      }
      _ => value.push('\\'),
    }
  }
  value
}

/// The character a single-letter ANSI-C escape stands for.
fn simple_escape(letter: char) -> Option<char> {
  Some(match letter {
    'a' => '\u{7}',
    'b' => '\u{8}',
    'e' | 'E' => '\u{1b}',
    'f' => '\u{c}',
    'n' => '\n',
    'r' => '\r',
    't' => '\t',
    'v' => '\u{b}',
    '\\' | '\'' | '"' | '?' => letter,
    _ => return None,
  })
}

/// Take up to `max` digits of `radix` from the front of `rest`.
fn digits(rest: &str, radix: u32, max: usize) -> &str {
  let len = rest
    .char_indices()
    .take(max)
    .take_while(|(_, c)| c.is_digit(radix))
    .last()
    .map_or(0, |(at, c)| at + c.len_utf8());
  rest.get(..len).unwrap_or_default()
}

/// `\cX`: the control character, with unbash's handling of a backslash operand.
fn control(rest: &str, value: &mut String) -> usize {
  let mut chars = rest.chars();
  match chars.next() {
    None => {
      value.push_str("\\c");
      0
    }
    Some('\\') => {
      value.push('\u{1c}');
      match chars.next() {
        Some('\\') => 2,
        Some(next) => {
          value.push(next);
          1 + next.len_utf8()
        }
        None => 1,
      }
    }
    Some(c) => {
      let code = if c == '?' { 127 } else { u32::from(c) & 31 };
      value.extend(char::from_u32(code));
      c.len_utf8()
    }
  }
}

/// One escape after a backslash; returns how many bytes of `rest` it used.
fn ansi_escape(letter: char, rest: &str, value: &mut String) -> usize {
  if let Some(c) = simple_escape(letter) {
    value.push(c);
    return 0;
  }
  let (radix, max) = match letter {
    '\n' => return 0,
    'c' => return control(rest, value),
    'x' => (16, 2),
    'u' => (16, 4),
    'U' => (16, 8),
    '0'..='7' => (8, 2),
    _ => {
      value.push('\\');
      value.push(letter);
      return 0;
    }
  };
  let found = digits(rest, radix, max);
  if radix == 16 && found.is_empty() {
    value.push('\\');
    value.push(letter);
    return 0;
  }
  let code = if radix == 8 {
    let mut octal = String::from(letter);
    octal.push_str(found);
    u32::from_str_radix(&octal, 8).unwrap_or(0) & 0xff
  } else {
    u32::from_str_radix(found, 16).unwrap_or(0)
  };
  if let Some(c) = char::from_u32(code) {
    value.push(c);
  } else {
    value.push('\\');
    value.push(letter);
    value.push_str(found);
  }
  found.len()
}

/// The body of `$'…'` (between the quotes), decoded.
pub(crate) fn ansi_c(body: &str) -> String {
  let mut value = String::with_capacity(body.len());
  let mut rest = body;
  while let Some(at) = rest.find('\\') {
    value.push_str(rest.get(..at).unwrap_or_default());
    let after = rest.get(at + 1..).unwrap_or_default();
    let mut chars = after.chars();
    let Some(letter) = chars.next() else {
      value.push('\\');
      return value;
    };
    let tail = chars.as_str();
    let used = ansi_escape(letter, tail, &mut value);
    rest = tail.get(used..).unwrap_or_default();
  }
  value.push_str(rest);
  value
}

/// Unquoted `*`, `?` or `[…]`: bash replaces the word with the existing file it matches.
pub(crate) fn has_glob(raw: &str) -> bool {
  let mut plain = String::with_capacity(raw.len());
  let mut chars = raw.chars();
  while let Some(c) = chars.next() {
    if c == '\\' {
      chars.next();
    } else {
      plain.push(c);
    }
  }
  if plain.contains(['*', '?']) {
    return true;
  }
  plain
    .find('[')
    .and_then(|open| plain.get(open + 1..))
    .is_some_and(|after| after.contains(']'))
}

/// The body of a backtick substitution as bash reads it: a backslash before
/// `$`, a backtick or another backslash is dropped; any other backslash stays.
pub(crate) fn backticks(body: &str) -> String {
  let mut decoded = String::with_capacity(body.len());
  let mut chars = body.chars().peekable();
  while let Some(c) = chars.next() {
    match (c, chars.peek().copied()) {
      ('\\', Some(next @ ('$' | '`' | '\\'))) => {
        chars.next();
        decoded.push(next);
      }
      (c, _) => decoded.push(c),
    }
  }
  decoded
}

/// Unquoted brace expansion (`{a,b}`, `{1..3}`): one word becomes several, so it
/// has no static value. `{}` and `{a}` stay literal.
pub(crate) fn has_brace(raw: &str) -> bool {
  let mut opens: Vec<(usize, bool)> = Vec::new();
  let mut chars = raw.char_indices();
  while let Some((at, c)) = chars.next() {
    let closed = match c {
      '\\' => {
        chars.next();
        None
      }
      '{' => {
        opens.push((at, false));
        None
      }
      ',' => {
        opens.last_mut().into_iter().for_each(|open| open.1 = true);
        None
      }
      '}' => opens.pop().map(|(open, comma)| (open, at, comma)),
      _ => None,
    };
    let expands = closed.is_some_and(|(open, close, comma)| {
      comma
        || raw
          .get(open + 1..close)
          .is_some_and(|inside| inside.contains(".."))
    });
    if expands {
      return true;
    }
  }
  false
}

#[cfg(test)]
#[path = "tests/quote_test.rs"]
mod tests;
