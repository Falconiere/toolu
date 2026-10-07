//! Bash `case "$path" in $glob)` matching (`packages/toolu-core/src/ledger/glob.ts`),
//! as the docs-sync globs use it. `*` matches any run of characters (slashes
//! included), `?` one character, `[...]` a bracket expression (`!` or `^`
//! negates; ranges and `[:class:]` names work), and `\` quotes the next
//! character. An unclosed `[` is a literal. Character classes follow Unicode
//! categories as TypeScript's `\p{…}` does, approximated by Rust's `char` tests.

/// One element of a bracket expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
  /// One character.
  Char(char),
  /// An inclusive range.
  Range(char, char),
  /// A `[:name:]` class.
  Class(String),
}

/// One token of a compiled glob.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
  /// `*`.
  Star,
  /// `?`.
  One,
  /// A literal character.
  Literal(char),
  /// `[...]`, negated or not.
  Bracket(bool, Vec<Item>),
}

const CLASSES: [&str; 12] = [
  "alnum", "alpha", "blank", "cntrl", "digit", "graph", "lower", "print", "punct", "space",
  "upper", "xdigit",
];

fn punct(c: char) -> bool {
  c.is_ascii_punctuation()
    || (!c.is_ascii() && !c.is_alphanumeric() && !c.is_whitespace() && !c.is_control())
}

fn in_class(name: &str, c: char) -> bool {
  match name {
    "alnum" => c.is_alphanumeric(),
    "alpha" => c.is_alphabetic(),
    "blank" => c == ' ' || c == '\t',
    "cntrl" => c.is_control(),
    "digit" => c.is_ascii_digit(),
    "graph" => c.is_alphanumeric() || punct(c),
    "lower" => c.is_lowercase(),
    "print" => c.is_alphanumeric() || punct(c) || c == ' ',
    "punct" => punct(c),
    "space" => c.is_whitespace(),
    "upper" => c.is_uppercase(),
    _ => c.is_ascii_hexdigit(),
  }
}

/// The `[:name:]` class starting at `chars[at]`, and the index after it.
fn class_at(chars: &[char], at: usize) -> Option<(String, usize)> {
  if chars.get(at + 1) != Some(&':') {
    return None;
  }
  let close = (at + 2..chars.len()).find(|i| chars.get(*i) == Some(&':'))?;
  let name: String = chars.get(at + 2..close)?.iter().collect();
  (chars.get(close + 1) == Some(&']') && CLASSES.contains(&name.as_str()))
    .then_some((name, close + 2))
}

/// The bracket expression at `chars[start] == '['`: its token and the index of
/// its `]`, or `None` when it is never closed.
fn bracket(chars: &[char], start: usize) -> Option<(Token, usize)> {
  let mut i = start + 1;
  let negate = matches!(chars.get(i), Some('!' | '^'));
  if negate {
    i += 1;
  }
  let (mut items, mut first, mut range) = (Vec::new(), true, false);
  while let Some(&c) = chars.get(i) {
    if c == ']' && !first {
      return Some((Token::Bracket(negate, items), i));
    }
    first = false;
    let (item, next) = if let Some((name, next)) = (c == '[').then(|| class_at(chars, i)).flatten()
    {
      (Item::Class(name), next)
    } else if c == '\\' && i + 1 < chars.len() {
      (Item::Char(chars.get(i + 1).copied().unwrap_or(c)), i + 2)
    } else if c == '-' && !items.is_empty() && i + 1 < chars.len() && chars.get(i + 1) != Some(&']')
    {
      range = true;
      i += 1;
      continue;
    } else {
      (Item::Char(c), i + 1)
    };
    match (range, items.last(), item) {
      (true, Some(Item::Char(low)), Item::Char(high)) => {
        let low = *low;
        items.pop();
        items.push(Item::Range(low, high));
      }
      (_, _, item) => items.push(item),
    }
    range = false;
    i = next;
  }
  None
}

fn compile(pattern: &str) -> Vec<Token> {
  let chars: Vec<char> = pattern.chars().collect();
  let mut tokens = Vec::new();
  let mut i = 0;
  while let Some(&c) = chars.get(i) {
    let token = match c {
      '*' => Token::Star,
      '?' => Token::One,
      '\\' if i + 1 < chars.len() => {
        i += 1;
        Token::Literal(chars.get(i).copied().unwrap_or(c))
      }
      '[' => match bracket(&chars, i) {
        Some((token, end)) => {
          i = end;
          token
        }
        None => Token::Literal('['),
      },
      _ => Token::Literal(c),
    };
    tokens.push(token);
    i += 1;
  }
  tokens
}

fn single(token: &Token, c: char) -> bool {
  match token {
    Token::Star | Token::One => true,
    Token::Literal(own) => *own == c,
    Token::Bracket(negate, items) => {
      let hit = items.iter().any(|item| match item {
        Item::Char(own) => *own == c,
        Item::Range(low, high) => (*low..=*high).contains(&c),
        Item::Class(name) => in_class(name, c),
      });
      hit != *negate
    }
  }
}

/// Whether `path` matches the whole of `pattern`.
pub fn matches(path: &str, pattern: &str) -> bool {
  let tokens = compile(pattern);
  let text: Vec<char> = path.chars().collect();
  let (mut t, mut p) = (0, 0);
  let mut resume: Option<(usize, usize)> = None;
  while t < text.len() {
    match tokens.get(p) {
      Some(Token::Star) => {
        resume = Some((p, t));
        p += 1;
      }
      Some(token) if text.get(t).is_some_and(|c| single(token, *c)) => {
        t += 1;
        p += 1;
      }
      _ => match resume {
        Some((star, at)) => {
          p = star + 1;
          t = at + 1;
          resume = Some((star, at + 1));
        }
        None => return false,
      },
    }
  }
  tokens
    .get(p..)
    .is_some_and(|rest| rest.iter().all(|token| *token == Token::Star))
}

/// `_vd_matches_any PATH GLOBS`: whether `path` matches a non-empty glob.
pub fn matches_any(path: &str, globs: &[String]) -> bool {
  globs
    .iter()
    .any(|glob| !glob.is_empty() && matches(path, glob))
}

#[cfg(test)]
#[path = "tests/glob_test.rs"]
mod tests;
