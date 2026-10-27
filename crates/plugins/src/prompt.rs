//! Interactive choices on the terminal (`/dev/tty`), never stdout, so a verb's
//! stdout stays its report. At a terminal that can go raw the choice is a
//! checkbox picker on rails; otherwise it is the numbered prompt that replaced
//! the TypeScript installer's `@clack/prompts`.

use std::fs::OpenOptions;
use std::io::{BufReader, IsTerminal as _};
use std::os::fd::AsFd as _;

use toolu_runtime::term::Style;

use crate::fail::Res;

/// The checkbox picker.
pub(crate) mod checkbox;
/// Keys read in raw mode.
pub(crate) mod keys;
/// The numbered prompt.
pub(crate) mod numbered;
/// Raw mode for one prompt.
pub(crate) mod raw;

/// An option: its label and an optional hint.
pub(crate) type Choice = (String, Option<String>);

/// Something that can ask the user to choose.
pub(crate) trait Ask {
  /// Several of `options`, all chosen by default; the chosen indices.
  fn many(&mut self, message: &str, options: &[Choice]) -> Res<Vec<usize>>;
  /// One of `options`, the first by default; its index.
  fn one(&mut self, message: &str, options: &[Choice]) -> Res<usize>;
}

/// The terminal prompts, when this run may ask: input allowed, stdin a
/// terminal, and `/dev/tty` open. The checkbox picker when `style` draws rails
/// and the terminal's attributes can be read; the numbered prompt otherwise.
pub(crate) fn terminal(allowed: bool, style: Style) -> Option<Box<dyn Ask>> {
  if !allowed || !std::io::stdin().is_terminal() {
    return None;
  }
  let tty = OpenOptions::new()
    .read(true)
    .write(true)
    .open("/dev/tty")
    .ok()?;
  if style.rails() && raw::probe(tty.as_fd()) {
    return Some(Box::new(checkbox::Checkbox::new(tty, style)));
  }
  let input = tty.try_clone().ok()?;
  Some(Box::new(numbered::Prompter::new(
    BufReader::new(input),
    tty,
  )))
}

#[cfg(test)]
#[path = "tests/prompt_test.rs"]
mod tests;
