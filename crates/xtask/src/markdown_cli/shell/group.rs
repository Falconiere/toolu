//! Parentheses and `$`: function definitions, subshells, `$(…)` and `<(…)`
//! substitutions that run inside their outer command, array assignments and
//! `${…}` words.

use super::Lexer;

impl Lexer {
  /// `name()` defines a function, `name=(…)` assigns an array; any other
  /// `(` starts a command.
  pub(super) fn open_paren(&mut self) {
    if self
      .word
      .as_ref()
      .is_some_and(|(word, _)| word.ends_with('='))
    {
      self.push_array();
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

  /// An array's `(…)`, which may span lines, is part of its assignment word.
  pub(super) fn push_array(&mut self) {
    self.push('(');
    while let Some(c) = self.peek(0) {
      self.pos += 1;
      if c == '\n' {
        self.line += 1;
      }
      self.push(c);
      if c == ')' {
        return;
      }
    }
  }

  /// Run the inner command of a substitution, then return to the outer one.
  pub(super) fn open_substitution(&mut self) {
    self.end_word();
    self.outer.push(std::mem::take(&mut self.words));
  }

  pub(super) fn close_paren(&mut self) {
    self.end_command();
    if self.subshells > 0 {
      self.subshells -= 1;
    } else if let Some(words) = self.outer.pop() {
      self.words = words;
    }
  }

  /// `$(` starts a command; `${…}` and `$name` stay in the word.
  pub(super) fn dollar(&mut self) {
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

#[cfg(test)]
#[path = "tests/group_test.rs"]
mod tests;
