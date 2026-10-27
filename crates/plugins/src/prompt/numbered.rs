//! The numbered prompt: options as numbered lines, answered by typing numbers.
//! It is the fallback where the checkbox cannot put the terminal in raw mode,
//! and its logic runs over any `BufRead` and `Write`, which is how it is tested.

use std::fmt::Write as _;
use std::io::{BufRead, Write};

use crate::fail::{Fail, Res};
use crate::prompt::{Ask, Choice};

/// Prompts over a reader and a writer.
#[derive(Debug)]
pub(crate) struct Prompter<R, W> {
  input: R,
  output: W,
}

impl<R: BufRead, W: Write> Prompter<R, W> {
  /// Prompts that read `input` and write `output`.
  pub(crate) fn new(input: R, output: W) -> Self {
    Prompter { input, output }
  }

  fn show(&mut self, message: &str, options: &[Choice], hint: &str) -> Res<()> {
    let mut text = format!("{message}\n");
    for (index, (label, about)) in options.iter().enumerate() {
      let about = about
        .as_deref()
        .map(|about| format!(" - {about}"))
        .unwrap_or_default();
      let _written = writeln!(text, "  {}) {label}{about}", index + 1);
    }
    text.push_str(hint);
    self.say(&text)
  }

  fn say(&mut self, text: &str) -> Res<()> {
    self
      .output
      .write_all(text.as_bytes())
      .and_then(|()| self.output.flush())
      .map_err(|err| Fail::failed(format!("cannot prompt: {err}")))
  }

  /// The next answer, trimmed; end of input or `q` cancels.
  fn answer(&mut self) -> Res<String> {
    let mut line = String::new();
    let read = self
      .input
      .read_line(&mut line)
      .map_err(|err| Fail::failed(format!("cannot prompt: {err}")))?;
    let answer = line.trim();
    if read == 0 || answer == "q" {
      return Err(Fail::failed("cancelled"));
    }
    Ok(answer.to_owned())
  }
}

/// The 0-based indices `answer` names, or `None` when one is not 1..=`count`.
fn picks(answer: &str, count: usize) -> Option<Vec<usize>> {
  let mut chosen = Vec::new();
  for word in answer.split([' ', ',']).filter(|word| !word.is_empty()) {
    let number: usize = word.parse().ok()?;
    let index = number.checked_sub(1).filter(|index| *index < count)?;
    if !chosen.contains(&index) {
      chosen.push(index);
    }
  }
  Some(chosen).filter(|chosen| !chosen.is_empty())
}

impl<R: BufRead, W: Write> Ask for Prompter<R, W> {
  fn many(&mut self, message: &str, options: &[Choice]) -> Res<Vec<usize>> {
    self.show(
      message,
      options,
      "Numbers to choose (Enter: all, q: cancel): ",
    )?;
    loop {
      let answer = self.answer()?;
      if answer.is_empty() {
        return Ok((0..options.len()).collect());
      }
      if let Some(chosen) = picks(&answer, options.len()) {
        return Ok(chosen);
      }
      self.say(&format!(
        "Answer with numbers from 1 to {}: ",
        options.len()
      ))?;
    }
  }

  fn one(&mut self, message: &str, options: &[Choice]) -> Res<usize> {
    self.show(message, options, "Number to choose (Enter: 1, q: cancel): ")?;
    loop {
      let answer = self.answer()?;
      if answer.is_empty() {
        return Ok(0);
      }
      if let Some([index]) = picks(&answer, options.len()).as_deref() {
        return Ok(*index);
      }
      self.say(&format!(
        "Answer with one number from 1 to {}: ",
        options.len()
      ))?;
    }
  }
}

#[cfg(test)]
#[path = "tests/numbered_test.rs"]
mod tests;
