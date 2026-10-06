//! Redirections and heredocs: a redirection drops with its target, and a
//! heredoc's body is skipped at the end of its line.

use super::Lexer;
use crate::markdown_cli::words::unquote;

impl Lexer {
  /// `<name>` is a placeholder word; `<<` opens a heredoc; any other `<` or
  /// `>` redirects, dropping a numeric descriptor before it and its target.
  pub(super) fn redirect(&mut self, c: char) {
    if c == '<' && self.placeholder_ahead() {
      self.push('<');
      self.push_through('>');
      return;
    }
    if self
      .word
      .as_ref()
      .is_some_and(|(w, _)| w.chars().all(|d| d.is_ascii_digit()))
    {
      self.word = None;
    }
    self.end_word();
    if c == '<' && self.peek(0) == Some('<') && self.peek(1) != Some('<') {
      self.pos += 1;
      self.heredoc();
      return;
    }
    if c == '<' && self.peek(0) == Some('(') {
      self.pos += 1;
      self.open_substitution();
      return;
    }
    while self
      .peek(0)
      .is_some_and(|n| matches!(n, '<' | '>' | '&' | '|'))
    {
      self.pos += 1;
    }
    self.target();
  }

  /// `<` followed by a letter or `[`, up to a `>` before any whitespace.
  fn placeholder_ahead(&self) -> bool {
    if !self
      .peek(0)
      .is_some_and(|c| c.is_alphabetic() || c == '[' || c == '.')
    {
      return false;
    }
    let rest = self.chars.iter().skip(self.pos);
    for c in rest {
      match c {
        '>' => return true,
        c if c.is_whitespace() || *c == '<' => return false,
        _ => {}
      }
    }
    false
  }

  /// Skip the redirection target: one word, possibly quoted or a descriptor.
  fn target(&mut self) {
    while self.peek(0).is_some_and(|c| c == ' ' || c == '\t') {
      self.pos += 1;
    }
    let before = self.words.len();
    while let Some(c) = self.peek(0) {
      if c.is_whitespace() || matches!(c, ';' | '|' | ')' | '&' | '<' | '>') {
        break;
      }
      self.pos += 1;
      self.step(c);
    }
    self.word = None;
    self.words.truncate(before);
  }

  /// Read the delimiter after `<<` or `<<-`; its body is skipped at the newline.
  fn heredoc(&mut self) {
    if self.peek(0) == Some('-') {
      self.pos += 1;
    }
    while self.peek(0).is_some_and(|c| c == ' ') {
      self.pos += 1;
    }
    let mut delimiter = String::new();
    while let Some(c) = self
      .peek(0)
      .filter(|c| !c.is_whitespace() && !matches!(c, ';' | '|' | '&' | ')'))
    {
      self.pos += 1;
      delimiter.push(c);
    }
    self.heredocs.push(unquote(&delimiter));
  }

  pub(super) fn skip_heredoc(&mut self, delimiter: &str) {
    while self.peek(0).is_some() {
      let start = self.pos;
      self.skip_line();
      let text: String = self
        .chars
        .iter()
        .skip(start)
        .take(self.pos - start)
        .collect();
      if self.peek(0) == Some('\n') {
        self.pos += 1;
        self.line += 1;
      }
      if text.trim() == delimiter {
        return;
      }
    }
  }
}

#[cfg(test)]
#[path = "tests/redirect_test.rs"]
mod tests;
