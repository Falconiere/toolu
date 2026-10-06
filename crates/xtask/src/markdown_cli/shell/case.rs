//! `case … esac`: an arm's `pattern)` runs nothing, its commands run, and
//! `;;` ends the arm.

use super::Lexer;

impl Lexer {
  /// `;;` ends a `case` arm; any other `;` ends the command.
  pub(super) fn semicolon(&mut self) {
    if self.cases > 0 && self.peek(0) == Some(';') {
      self.pos += 1;
      self.arm = false;
    }
    self.end_command();
  }

  /// A `case` arm's `pattern)` starts here: inside a `case`, between arms, at
  /// a command start, on a line with a `)` that is not `esac`.
  pub(super) fn at_pattern(&self, c: char) -> bool {
    if self.cases == 0
      || self.arm
      || self.word.is_some()
      || !self.words.is_empty()
      || c.is_whitespace()
      || c == '#'
    {
      return false;
    }
    let rest: String = self
      .chars
      .iter()
      .skip(self.pos - 1)
      .take_while(|c| **c != '\n')
      .collect();
    rest.contains(')') && !rest.starts_with("esac")
  }

  /// Skip the pattern through its `)`; the arm's commands follow.
  pub(super) fn skip_pattern(&mut self, c: char) {
    if c != ')' {
      while self.peek(0).is_some_and(|next| next != ')') {
        self.pos += 1;
      }
      self.pos += 1;
    }
    self.arm = true;
  }
}

#[cfg(test)]
#[path = "tests/case_test.rs"]
mod tests;
