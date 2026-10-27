//! The checkbox picker: options on rails at the terminal, moved with ↑/↓ (or
//! `k`/`j`), toggled with space, all at once with `a`, confirmed with Enter and
//! cancelled with `q`, Esc or Ctrl-C. Raw mode lasts one prompt; where it cannot
//! start, the numbered prompt asks instead. Once answered, the list collapses to
//! one line naming the choice.

use std::fs::File;
use std::io::{BufReader, Write as _};

use toolu_runtime::term::{Style, Tone, paint, truncate};

use crate::fail::{Fail, Res};
use crate::prompt::keys::{Key, read_key};
use crate::prompt::numbered::Prompter;
use crate::prompt::raw::Raw;
use crate::prompt::{Ask, Choice};

/// The widest a row may be drawn: a wrapped row would break the redraw.
const ROW_COLS: usize = 76;

/// The picker's state: the cursor and, for several, what is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pick {
  /// The row under the cursor.
  pub(crate) cursor: usize,
  /// Each option's checkbox; unused when only one is picked.
  pub(crate) chosen: Vec<bool>,
  /// Several (checkboxes) or one (the cursor row).
  pub(crate) many: bool,
  /// Enter was pressed with nothing chosen.
  pub(crate) empty: bool,
}

/// How a key press ended the prompt, if it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Done {
  /// These indices were chosen.
  Chose(Vec<usize>),
  /// The person cancelled.
  Cancelled,
}

impl Pick {
  /// `count` options, all chosen.
  pub(crate) fn new(count: usize, many: bool) -> Pick {
    Pick {
      cursor: 0,
      chosen: vec![true; count],
      many,
      empty: false,
    }
  }

  /// Applies `key`; `Some` when it ends the prompt.
  pub(crate) fn press(&mut self, key: Key) -> Option<Done> {
    let count = self.chosen.len().max(1);
    self.empty = false;
    match key {
      Key::Up => self.cursor = (self.cursor + count - 1) % count,
      Key::Down => self.cursor = (self.cursor + 1) % count,
      Key::Toggle if self.many => {
        if let Some(slot) = self.chosen.get_mut(self.cursor) {
          *slot = !*slot;
        }
      }
      Key::All if self.many => {
        let all = self.chosen.iter().all(|chosen| *chosen);
        self.chosen.iter_mut().for_each(|chosen| *chosen = !all);
      }
      Key::Enter if self.many => {
        let picked: Vec<usize> = (0..self.chosen.len())
          .filter(|at| self.chosen.get(*at) == Some(&true))
          .collect();
        if !picked.is_empty() {
          return Some(Done::Chose(picked));
        }
        self.empty = true;
      }
      Key::Enter => return Some(Done::Chose(vec![self.cursor])),
      Key::Cancel => return Some(Done::Cancelled),
      Key::Toggle | Key::All | Key::Other => {}
    }
    None
  }
}

/// The open picker: `◆  message`, a row per option, and the keys.
pub(crate) fn render(style: Style, message: &str, options: &[Choice], pick: &Pick) -> String {
  let width = options
    .iter()
    .map(|(label, _)| label.chars().count())
    .max()
    .unwrap_or(0);
  let rail = paint(style, Tone::Dim, "│");
  let mut lines = vec![format!("{}  {message}", paint(style, Tone::Accent, "◆"))];
  for (at, (label, hint)) in options.iter().enumerate() {
    let here = at == pick.cursor;
    let mark = match (pick.many, pick.chosen.get(at) == Some(&true), here) {
      (true, true, _) => "◼",
      (true, false, _) => "◻",
      (false, _, true) => "●",
      (false, _, false) => "○",
    };
    let text = truncate(
      &format!(
        "{mark} {label:<width$}  {}",
        hint.as_deref().unwrap_or_default()
      ),
      ROW_COLS - 5,
    );
    let text = text.trim_end();
    let shown = if here {
      format!(
        "{} {}",
        paint(style, Tone::Accent, ">"),
        paint(style, Tone::Accent, text)
      )
    } else {
      format!("  {text}")
    };
    lines.push(format!("{rail}  {shown}"));
  }
  let keys = match (pick.empty, pick.many) {
    (true, _) => paint(style, Tone::Warn, "choose at least one"),
    (false, true) => "↑/↓ move · space toggle · a all · enter".to_owned(),
    (false, false) => "↑/↓ move · enter".to_owned(),
  };
  lines.push(format!("{}  {keys}", paint(style, Tone::Dim, "└")));
  lines.join("\n")
}

/// The answered picker: `◇  message  answer`.
pub(crate) fn collapsed(style: Style, message: &str, answer: &str) -> String {
  format!(
    "{}  {message}  {}\n{}\n",
    paint(style, Tone::Accent, "◇"),
    paint(style, Tone::Dim, answer),
    paint(style, Tone::Dim, "│")
  )
}

/// The checkbox picker on a terminal.
#[derive(Debug)]
pub(crate) struct Checkbox {
  tty: File,
  style: Style,
}

impl Checkbox {
  /// A picker on `tty`; raw mode starts only while a prompt is open.
  pub(crate) fn new(tty: File, style: Style) -> Checkbox {
    Checkbox { tty, style }
  }

  fn say(&self, text: &str) -> Res<()> {
    (&self.tty)
      .write_all(text.as_bytes())
      .and_then(|()| (&self.tty).flush())
      .map_err(|err| Fail::failed(format!("cannot prompt: {err}")))
  }

  /// Runs one prompt; the numbered prompt when raw mode cannot start.
  fn ask(&mut self, message: &str, options: &[Choice], many: bool) -> Res<Vec<usize>> {
    let Ok(mut raw) = Raw::enter(&self.tty) else {
      return self.numbered(message, options, many);
    };
    let mut pick = Pick::new(options.len(), many);
    let mut drawn = render(self.style, message, options, &pick);
    self.say(&drawn)?;
    let done = loop {
      let key = read_key(&mut raw).map_err(|err| Fail::failed(format!("cannot prompt: {err}")))?;
      if let Some(done) = pick.press(key) {
        break done;
      }
      let next = render(self.style, message, options, &pick);
      self.say(&format!("{}{next}", clear(&drawn)))?;
      drawn = next;
    };
    drop(raw);
    let (answer, result) = match done {
      Done::Chose(picked) => {
        let labels: Vec<&str> = picked
          .iter()
          .filter_map(|at| options.get(*at).map(|(label, _)| label.as_str()))
          .collect();
        (labels.join(", "), Ok(picked))
      }
      Done::Cancelled => ("cancelled".to_owned(), Err(Fail::failed("cancelled"))),
    };
    self.say(&format!(
      "{}{}",
      clear(&drawn),
      collapsed(self.style, message, &answer)
    ))?;
    result
  }

  fn numbered(&self, message: &str, options: &[Choice], many: bool) -> Res<Vec<usize>> {
    let clone = |tty: &File| {
      tty
        .try_clone()
        .map_err(|err| Fail::failed(format!("cannot prompt: {err}")))
    };
    let mut prompter = Prompter::new(BufReader::new(clone(&self.tty)?), clone(&self.tty)?);
    if many {
      prompter.many(message, options)
    } else {
      prompter.one(message, options).map(|at| vec![at])
    }
  }
}

impl Ask for Checkbox {
  fn many(&mut self, message: &str, options: &[Choice]) -> Res<Vec<usize>> {
    self.ask(message, options, true)
  }

  fn one(&mut self, message: &str, options: &[Choice]) -> Res<usize> {
    let picked = self.ask(message, options, false)?;
    picked
      .first()
      .copied()
      .ok_or_else(|| Fail::failed("cancelled"))
  }
}

/// Moves to the first line of `drawn` and clears from there down.
fn clear(drawn: &str) -> String {
  let up = drawn.matches('\n').count();
  if up == 0 {
    "\r\x1b[J".to_owned()
  } else {
    format!("\r\x1b[{up}A\x1b[J")
  }
}

#[cfg(test)]
#[path = "tests/checkbox_test.rs"]
mod tests;
