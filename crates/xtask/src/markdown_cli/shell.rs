//! Enough of a shell lexer to find the commands a Markdown block runs: words
//! keep their quotes, a quoted word may span lines, heredoc bodies and
//! comments are skipped, redirections drop with their target, and commands
//! end at newlines, `;`, `&`, `&&`, `||`, `|`, parentheses, braces, `$(` and
//! backticks. Leading assignments and keywords (`if`, `then`, `do`, …) are
//! dropped, so each command starts at the word the shell would run.

use super::words::{Lexed, command};

/// Lex `text`, whose first line is Markdown line `first_line`.
pub(crate) fn lex(text: &str, first_line: usize) -> Lexed {
  let mut lexer = Lexer {
    chars: text.chars().collect(),
    pos: 0,
    line: first_line,
    word: None,
    words: Vec::new(),
    heredocs: Vec::new(),
    outer: Vec::new(),
    subshells: 0,
    cases: 0,
    arm: false,
    out: Lexed::default(),
  };
  lexer.run();
  lexer.out
}

struct Lexer {
  chars: Vec<char>,
  pos: usize,
  line: usize,
  /// The word being built and its line.
  word: Option<(String, usize)>,
  /// The current command's words with their lines.
  words: Vec<(String, usize)>,
  /// Heredoc delimiters whose bodies start after the current line.
  heredocs: Vec<String>,
  /// The outer commands' words while a `$(…)` or `<(…)` runs.
  outer: Vec<Vec<(String, usize)>>,
  /// Open `(` subshells, closed by `)` before any substitution is.
  subshells: usize,
  /// Open `case … esac` constructs.
  cases: usize,
  /// Inside a `case` arm, after its `pattern)` and before its `;;`.
  arm: bool,
  out: Lexed,
}

impl Lexer {
  fn peek(&self, ahead: usize) -> Option<char> {
    self.chars.get(self.pos + ahead).copied()
  }

  fn run(&mut self) {
    while let Some(c) = self.peek(0) {
      self.pos += 1;
      self.step(c);
    }
    self.end_command();
  }

  fn step(&mut self, c: char) {
    if self.at_pattern(c) {
      self.skip_pattern(c);
      return;
    }
    match c {
      '\n' => self.newline(),
      ' ' | '\t' | '\r' => self.end_word(),
      '\\' => self.escape(),
      '\'' | '"' => self.quoted(c),
      '#' if self.word.is_none() => self.skip_line(),
      ')' => self.close_paren(),
      ';' => self.semicolon(),
      '&' | '|' | '`' | '{' | '}' if self.separates(c) => self.end_command(),
      '(' => self.open_paren(),
      '<' | '>' => self.redirect(c),
      '$' => self.dollar(),
      _ => self.push(c),
    }
  }

  /// `{` and `}` separate only as words of their own; the rest always do.
  fn separates(&self, c: char) -> bool {
    if c != '{' && c != '}' {
      return true;
    }
    self.word.is_none() && self.peek(0).is_none_or(char::is_whitespace)
  }

  fn push(&mut self, c: char) {
    let line = self.line;
    self
      .word
      .get_or_insert_with(|| (String::new(), line))
      .0
      .push(c);
  }

  fn end_word(&mut self) {
    if let Some(word) = self.word.take() {
      self.words.push(word);
    }
  }

  fn end_command(&mut self) {
    self.end_word();
    let words = std::mem::take(&mut self.words);
    match words.as_slice() {
      [(keyword, _), (name, _), ..] if keyword == "function" => {
        self.out.functions.push(name.clone());
      }
      [(keyword, _), ..] if keyword == "case" => {
        self.cases += 1;
        self.arm = false;
      }
      [(keyword, _), ..] if keyword == "esac" => self.cases = self.cases.saturating_sub(1),
      _ => {}
    }
    if let Some(command) = command(words) {
      self.out.commands.push(command);
    }
  }

  fn newline(&mut self) {
    self.end_command();
    self.line += 1;
    for delimiter in std::mem::take(&mut self.heredocs) {
      self.skip_heredoc(&delimiter);
    }
  }

  /// A backslash joins lines outside quotes and escapes the next character.
  fn escape(&mut self) {
    match self.peek(0) {
      Some('\n') => {
        self.pos += 1;
        self.line += 1;
      }
      Some(next) => {
        self.pos += 1;
        self.push(next);
      }
      None => {}
    }
  }

  /// A quoted run, quotes kept; it may span lines.
  fn quoted(&mut self, quote: char) {
    self.push(quote);
    while let Some(c) = self.peek(0) {
      self.pos += 1;
      if c == '\n' {
        self.line += 1;
      }
      if c == '\\' && quote == '"' {
        self.escaped();
        continue;
      }
      self.push(c);
      if c == quote {
        return;
      }
    }
  }

  /// The character after a backslash inside double quotes, taken literally.
  fn escaped(&mut self) {
    if let Some(next) = self.peek(0) {
      self.pos += 1;
      self.push(next);
    }
  }

  /// Push characters up to and including `end`, stopping before a newline.
  fn push_through(&mut self, end: char) {
    while let Some(c) = self.peek(0).filter(|c| *c != '\n') {
      self.pos += 1;
      self.push(c);
      if c == end {
        break;
      }
    }
  }

  fn skip_line(&mut self) {
    while self.peek(0).is_some_and(|c| c != '\n') {
      self.pos += 1;
    }
  }

  /// `name()` defines a function, `name=(…)` assigns an array; any other
  /// `(` starts a command.
  fn open_paren(&mut self) {
    if self
      .word
      .as_ref()
      .is_some_and(|(word, _)| word.ends_with('='))
    {
      self.push('(');
      self.push_through(')');
      return;
    }
    self.end_word();
    if self.peek(0) == Some(')') {
      self.pos += 1;
      if let Some((name, _)) = self.words.pop() {
        self.out.functions.push(name);
      }
    } else {
      self.subshells += 1;
    }
    self.end_command();
  }

  /// Run the inner command of a substitution, then return to the outer one.
  fn open_substitution(&mut self) {
    self.end_word();
    self.outer.push(std::mem::take(&mut self.words));
  }

  fn close_paren(&mut self) {
    self.end_command();
    if self.subshells > 0 {
      self.subshells -= 1;
    } else if let Some(words) = self.outer.pop() {
      self.words = words;
    }
  }

  /// `$(` starts a command; `${…}` and `$name` stay in the word.
  fn dollar(&mut self) {
    match self.peek(0) {
      Some('(') => {
        self.pos += 1;
        self.open_substitution();
      }
      Some('{') => {
        self.push('$');
        self.push_through('}');
      }
      _ => self.push('$'),
    }
  }
}

mod case;
mod redirect;

#[cfg(test)]
#[path = "tests/shell_test.rs"]
mod tests;
