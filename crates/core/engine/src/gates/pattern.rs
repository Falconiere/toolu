//! Bash `[[ text == pattern ]]` matching with extglob enabled. `*` and `?`
//! cross `/`, and brackets and extglobs match the whole string.

use std::collections::HashMap;

#[derive(Clone)]
enum Member {
  Char(char),
  Range(char, char),
  Class(String),
  Name(String),
}

impl Member {
  fn accepts(&self, value: char) -> bool {
    match self {
      Member::Char(want) => value == *want,
      Member::Range(low, high) => *low <= value && value <= *high,
      Member::Class(name) => class_accepts(name, value),
      Member::Name(name) => value.to_string() == *name,
    }
  }
}

fn class_accepts(name: &str, c: char) -> bool {
  match name {
    "ascii" => c.is_ascii(),
    "alnum" => c.is_alphanumeric(),
    "alpha" => c.is_alphabetic(),
    "blank" => matches!(c, ' ' | '\t'),
    "cntrl" => c.is_control(),
    "digit" => c.is_ascii_digit(),
    "graph" => !c.is_whitespace() && !c.is_control(),
    "lower" => c.is_lowercase(),
    "print" => !c.is_control(),
    "punct" => c.is_ascii_punctuation(),
    "space" => c.is_whitespace(),
    "upper" => c.is_uppercase(),
    "word" => c.is_alphanumeric() || c == '_',
    "xdigit" => c.is_ascii_hexdigit(),
    _ => false,
  }
}

#[derive(Clone)]
enum Node {
  Literal(char),
  Any,
  Star,
  Bracket {
    negated: bool,
    members: Vec<Member>,
  },
  Ext {
    op: char,
    alternatives: Vec<Vec<Node>>,
  },
}

/// One reusable parsed Bash pattern.
pub(crate) struct Pattern(Vec<Node>);

impl Pattern {
  /// Parse a Bash pattern for repeated matches.
  pub(crate) fn new(pattern: &str) -> Pattern {
    let chars: Vec<char> = pattern.chars().collect();
    Pattern(parse(&chars, 0, chars.len()))
  }

  /// Match the complete text against this pattern.
  pub(crate) fn matches(&self, text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    Matcher::new(&self.0, &chars, chars.len()).step(0, 0)
  }
}

/// Direct match for callers that do not reuse a pattern.
pub(crate) fn matches(pattern: &str, text: &str) -> bool {
  Pattern::new(pattern).matches(text)
}

/// One bracket member and the next offset. The optional char can start a range.
fn member(chars: &[char], at: usize) -> Option<(usize, Member, Option<char>)> {
  let c = *chars.get(at)?;
  if c == '['
    && let Some(named) = named_member(chars, at)
  {
    return Some(named);
  }
  if c == '\\' {
    let escaped = *chars.get(at + 1)?;
    return Some((at + 2, Member::Char(escaped), Some(escaped)));
  }
  Some((at + 1, Member::Char(c), Some(c)))
}

fn named_member(chars: &[char], at: usize) -> Option<(usize, Member, Option<char>)> {
  let kind = *chars.get(at + 1)?;
  if !matches!(kind, ':' | '=' | '.') {
    return None;
  }
  let close = chars
    .iter()
    .enumerate()
    .skip(at + 2)
    .find_map(|(i, item)| (*item == kind && chars.get(i + 1) == Some(&']')).then_some(i))?;
  let name: String = chars.get(at + 2..close)?.iter().collect();
  let character = (kind != ':').then(|| name.chars().next()).flatten();
  let item = if kind == ':' {
    Member::Class(name)
  } else {
    Member::Name(name)
  };
  Some((close + 2, item, character))
}

fn bracket(chars: &[char], start: usize) -> Option<(usize, Node)> {
  let mut at = start + 1;
  let negated = matches!(chars.get(at), Some('!' | '^'));
  if negated {
    at += 1;
  }
  let mut members = Vec::new();
  let mut first = true;
  while at < chars.len() {
    if chars.get(at) == Some(&']') && !first {
      return Some((at + 1, Node::Bracket { negated, members }));
    }
    first = false;
    let (next, item, low) = member(chars, at)?;
    if let Some(low) = low
      && chars.get(next) == Some(&'-')
      && chars.get(next + 1) != Some(&']')
      && let Some((after, _, Some(high))) = member(chars, next + 1)
    {
      members.push(Member::Range(low, high));
      at = after;
      continue;
    }
    members.push(item);
    at = next;
  }
  None
}

/// Find the closing `)` and top-level `|` offsets of an extglob.
fn ext_end(chars: &[char], start: usize) -> Option<(usize, Vec<usize>)> {
  let mut depth = 0;
  let mut bars = Vec::new();
  let mut at = start;
  while at < chars.len() {
    match chars.get(at) {
      Some('\\') => at += 2,
      Some('[') if bracket(chars, at).is_some() => at = bracket(chars, at)?.0,
      Some('(') => {
        depth += 1;
        at += 1;
      }
      Some(')') if depth == 0 => return Some((at, bars)),
      Some(')') => {
        depth -= 1;
        at += 1;
      }
      Some('|') if depth == 0 => {
        bars.push(at);
        at += 1;
      }
      Some(_) | None => at += 1,
    }
  }
  None
}

fn ext_node(chars: &[char], at: usize, to: usize) -> Option<(usize, Node)> {
  let op = *chars.get(at)?;
  if !matches!(op, '?' | '*' | '+' | '@' | '!') || chars.get(at + 1) != Some(&'(') {
    return None;
  }
  let (end, bars) = ext_end(chars, at + 2)?;
  if end >= to {
    return None;
  }
  let mut starts = vec![at + 1];
  starts.extend(bars.iter().copied());
  let mut ends = bars;
  ends.push(end);
  let alternatives = starts
    .into_iter()
    .zip(ends)
    .map(|(start, finish)| parse(chars, start + 1, finish))
    .collect();
  Some((end + 1, Node::Ext { op, alternatives }))
}

fn parse(chars: &[char], from: usize, to: usize) -> Vec<Node> {
  let mut nodes = Vec::new();
  let mut at = from;
  while at < to {
    let Some(&c) = chars.get(at) else { break };
    if let Some((end, node)) = ext_node(chars, at, to) {
      nodes.push(node);
      at = end;
      continue;
    }
    if c == '['
      && let Some((end, node)) = bracket(chars, at)
      && end <= to
    {
      nodes.push(node);
      at = end;
      continue;
    }
    if c == '\\'
      && at + 1 < to
      && let Some(&next) = chars.get(at + 1)
    {
      nodes.push(Node::Literal(next));
      at += 2;
      continue;
    }
    nodes.push(match c {
      '*' => Node::Star,
      '?' => Node::Any,
      _ => Node::Literal(c),
    });
    at += 1;
  }
  nodes
}

struct Matcher<'a> {
  nodes: &'a [Node],
  text: &'a [char],
  to: usize,
  memo: HashMap<(usize, usize), bool>,
}

impl<'a> Matcher<'a> {
  fn new(nodes: &'a [Node], text: &'a [char], to: usize) -> Self {
    Self {
      nodes,
      text,
      to,
      memo: HashMap::new(),
    }
  }

  fn any_alt(&self, alternatives: &[Vec<Node>], from: usize, to: usize) -> bool {
    alternatives
      .iter()
      .any(|nodes| Matcher::new(nodes, self.text, to).step(0, from))
  }

  fn repeat(
    &mut self,
    index: usize,
    pos: usize,
    alternatives: &[Vec<Node>],
    min_one: bool,
  ) -> bool {
    if !min_one && self.step(index + 1, pos) {
      return true;
    }
    (pos + 1..=self.to).any(|end| {
      self.any_alt(alternatives, pos, end)
        && (self.step(index + 1, end) || self.repeat(index, end, alternatives, false))
    })
  }

  fn ext(&mut self, index: usize, pos: usize, op: char, alternatives: &[Vec<Node>]) -> bool {
    match op {
      '!' => (pos..=self.to)
        .any(|end| !self.any_alt(alternatives, pos, end) && self.step(index + 1, end)),
      '*' | '+' => self.repeat(index, pos, alternatives, op == '+'),
      '?' | '@' => {
        (op == '?' && self.step(index + 1, pos))
          || (pos..=self.to)
            .any(|end| self.any_alt(alternatives, pos, end) && self.step(index + 1, end))
      }
      _ => false,
    }
  }

  fn step(&mut self, index: usize, pos: usize) -> bool {
    if let Some(result) = self.memo.get(&(index, pos)) {
      return *result;
    }
    let result = match self.nodes.get(index).cloned() {
      None => pos == self.to,
      Some(Node::Literal(want)) => {
        self.text.get(pos) == Some(&want) && self.step(index + 1, pos + 1)
      }
      Some(Node::Any) => pos < self.to && self.step(index + 1, pos + 1),
      Some(Node::Star) => (pos..=self.to).any(|end| self.step(index + 1, end)),
      Some(Node::Bracket { negated, members }) => {
        self
          .text
          .get(pos)
          .is_some_and(|c| members.iter().any(|m| m.accepts(*c)) != negated)
          && self.step(index + 1, pos + 1)
      }
      Some(Node::Ext { op, alternatives }) => self.ext(index, pos, op, &alternatives),
    };
    self.memo.insert((index, pos), result);
    result
  }
}

#[cfg(test)]
#[path = "tests/pattern_test.rs"]
mod tests;
