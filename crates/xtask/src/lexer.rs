//! Which lines of a Rust file are code, and what its comments say.
//!
//! Code lines are lines with any character outside a comment; blank lines and
//! comment-only lines (doc comments included) are not code. Strings, raw strings
//! and character literals are skipped, so `"/*"` or `'"'` never opens anything.

/// The lexed view of one file.
#[derive(Debug, Default)]
pub(crate) struct Lines {
  /// `code[i]` is true when line `i + 1` holds code.
  pub(crate) code: Vec<bool>,
  /// Every comment with the 1-based line it starts on.
  pub(crate) comments: Vec<(usize, String)>,
}

impl Lines {
  /// Code lines in the whole file.
  pub(crate) fn code_lines(&self) -> usize {
    self.code.iter().filter(|code| **code).count()
  }

  /// Code lines from `first` to `last`, both 1-based and inclusive.
  pub(crate) fn code_lines_between(&self, first: usize, last: usize) -> usize {
    self
      .code
      .iter()
      .enumerate()
      .filter(|(index, code)| **code && (first..=last).contains(&(index + 1)))
      .count()
  }
}

/// Lexer state between characters.
enum State {
  Code,
  LineComment,
  BlockComment(usize),
  Str,
  RawStr(usize),
}

struct Lexer<'a> {
  chars: std::iter::Peekable<std::str::Chars<'a>>,
  lines: Lines,
  line: usize,
  comment: String,
  comment_line: usize,
}

/// Lex `text`.
pub(crate) fn lex(text: &str) -> Lines {
  let mut lexer = Lexer {
    chars: text.chars().peekable(),
    lines: Lines {
      code: vec![false],
      comments: Vec::new(),
    },
    line: 0,
    comment: String::new(),
    comment_line: 1,
  };
  let mut state = State::Code;
  while let Some(c) = lexer.chars.next() {
    state = lexer.step(state, c);
  }
  if matches!(state, State::LineComment | State::BlockComment(_)) {
    lexer.end_comment();
  }
  lexer.lines
}

impl Lexer<'_> {
  fn step(&mut self, state: State, c: char) -> State {
    if c == '\n' {
      return self.newline(state);
    }
    match state {
      State::Code => self.code(c),
      State::LineComment => {
        self.comment.push(c);
        State::LineComment
      }
      State::BlockComment(depth) => self.block_comment(depth, c),
      State::Str => self.string(c),
      State::RawStr(hashes) => self.raw_string(hashes, c),
    }
  }

  fn newline(&mut self, state: State) -> State {
    self.line += 1;
    self.lines.code.push(false);
    match state {
      State::LineComment => {
        self.end_comment();
        State::Code
      }
      State::BlockComment(depth) => {
        self.comment.push('\n');
        State::BlockComment(depth)
      }
      State::Str | State::RawStr(_) => {
        self.mark_code();
        state
      }
      State::Code => State::Code,
    }
  }

  fn code(&mut self, c: char) -> State {
    if c.is_whitespace() {
      return State::Code;
    }
    if c == '/' && self.chars.peek() == Some(&'/') {
      self.chars.next();
      self.start_comment();
      return State::LineComment;
    }
    if c == '/' && self.chars.peek() == Some(&'*') {
      self.chars.next();
      self.start_comment();
      return State::BlockComment(1);
    }
    self.mark_code();
    match c {
      '"' => State::Str,
      'r' => self.raw_start(),
      '\'' => {
        self.char_literal();
        State::Code
      }
      _ => State::Code,
    }
  }

  /// After `r`: `r"…"` or `r#…#"…"#…#` opens a raw string; anything else is an identifier.
  fn raw_start(&mut self) -> State {
    let mut hashes = 0;
    while self.chars.peek() == Some(&'#') {
      self.chars.next();
      hashes += 1;
    }
    if self.chars.peek() == Some(&'"') {
      self.chars.next();
      return State::RawStr(hashes);
    }
    State::Code
  }

  /// After `'`: skip a character literal; a lifetime has no closing quote.
  fn char_literal(&mut self) {
    let mut ahead = self.chars.clone();
    match ahead.next() {
      Some('\\') => {
        self.chars.next();
        self.chars.next();
        let rest = self.chars.by_ref();
        rest.find(|c| *c == '\'' || *c == '\n');
      }
      Some(_) if ahead.next() == Some('\'') => {
        self.chars.next();
        self.chars.next();
      }
      _ => {}
    }
  }

  fn block_comment(&mut self, depth: usize, c: char) -> State {
    if c == '*' && self.chars.peek() == Some(&'/') {
      self.chars.next();
      if depth == 1 {
        self.end_comment();
        return State::Code;
      }
      return State::BlockComment(depth - 1);
    }
    if c == '/' && self.chars.peek() == Some(&'*') {
      self.chars.next();
      return State::BlockComment(depth + 1);
    }
    self.comment.push(c);
    State::BlockComment(depth)
  }

  fn string(&mut self, c: char) -> State {
    match c {
      '\\' => {
        if self.chars.next() == Some('\n') {
          self.newline(State::Str);
        }
        State::Str
      }
      '"' => State::Code,
      _ => State::Str,
    }
  }

  fn raw_string(&mut self, hashes: usize, c: char) -> State {
    if c != '"' {
      return State::RawStr(hashes);
    }
    let mut ahead = self.chars.clone();
    if (0..hashes).all(|_| ahead.next() == Some('#')) {
      for _ in 0..hashes {
        self.chars.next();
      }
      return State::Code;
    }
    State::RawStr(hashes)
  }

  fn mark_code(&mut self) {
    if let Some(code) = self.lines.code.get_mut(self.line) {
      *code = true;
    }
  }

  fn start_comment(&mut self) {
    self.comment.clear();
    self.comment_line = self.line + 1;
  }

  fn end_comment(&mut self) {
    let text = std::mem::take(&mut self.comment);
    self.lines.comments.push((self.comment_line, text));
  }
}

#[cfg(test)]
#[path = "tests/lexer_test.rs"]
mod tests;
