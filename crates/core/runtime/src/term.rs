//! Terminal styling for human output: the rails look `toolu plugins` and
//! `toolu status` share at a terminal. `toolu-cli` decides the [`Style`] once,
//! from whether stdout is a terminal, `TERM` and `NO_COLOR`; anything that is
//! not a terminal keeps the plain text it always had.

use crate::env::Env;

/// How human output is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Style {
  /// Not a terminal (a pipe, a file, `--json`, `TERM=dumb`): the plain text.
  #[default]
  Plain,
  /// A terminal under `NO_COLOR`: rails and glyphs, no ANSI escape at all.
  Mono,
  /// A terminal: rails and glyphs in color.
  Color,
}

impl Style {
  /// Whether the rails layout applies, which is any terminal.
  pub fn rails(self) -> bool {
    self != Style::Plain
  }
}

/// What a painted piece of text means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
  /// A heading or the current choice (cyan).
  Accent,
  /// Done as asked (green).
  Good,
  /// Left alone, needs a look (yellow).
  Warn,
  /// Failed (red).
  Bad,
  /// Secondary: rails, versions, unchanged things (dim).
  Dim,
}

/// The style for a stream: `Plain` unless it is a terminal whose `TERM` is not
/// `dumb`, then `Mono` under a non-empty `NO_COLOR` (no-color.org), else `Color`.
pub fn style_of(env: &Env, is_terminal: bool) -> Style {
  if !is_terminal || env.get("TERM") == Some("dumb") {
    return Style::Plain;
  }
  if env.get("NO_COLOR").is_some() {
    Style::Mono
  } else {
    Style::Color
  }
}

/// `text` in `tone`'s color under `Color`; unchanged otherwise.
pub fn paint(style: Style, tone: Tone, text: &str) -> String {
  if style != Style::Color || text.is_empty() {
    return text.to_owned();
  }
  let code = match tone {
    Tone::Accent => "36",
    Tone::Good => "32",
    Tone::Warn => "33",
    Tone::Bad => "31",
    Tone::Dim => "2",
  };
  format!("\x1b[{code}m{text}\x1b[0m")
}

fn rail(style: Style, glyph: &str) -> String {
  paint(style, Tone::Dim, glyph)
}

/// The opening of a report: `┌  title`, then an empty rail.
pub fn intro(style: Style, title: &str) -> String {
  format!("{}  {title}\n{}\n", rail(style, "┌"), rail(style, "│"))
}

/// A section heading: `◇  title`.
pub fn section(style: Style, title: &str) -> String {
  format!("{}  {title}\n", paint(style, Tone::Accent, "◇"))
}

/// One item: `│  <glyph> <name padded to width> <detail>`, the glyph in its tone.
pub fn row(
  style: Style,
  (glyph, tone): (&str, Tone),
  name: &str,
  width: usize,
  detail: &str,
) -> String {
  let text = format!("{name:<width$} {detail}");
  format!(
    "{}  {} {}\n",
    rail(style, "│"),
    paint(style, tone, glyph),
    text.trim_end()
  )
}

/// One labelled value: `│  <label padded to width><value>`.
pub fn label_row(style: Style, label: &str, width: usize, value: &str) -> String {
  format!("{}  {label:<width$}{value}\n", rail(style, "│"))
}

/// The close of a report: `└  text`, or a bare `└`.
pub fn outro(style: Style, text: &str) -> String {
  if text.is_empty() {
    format!("{}\n", rail(style, "└"))
  } else {
    format!("{}  {text}\n", rail(style, "└"))
  }
}

/// `text` cut to `cols` characters, the last one `…` when it was cut.
pub fn truncate(text: &str, cols: usize) -> String {
  if text.chars().count() <= cols {
    return text.to_owned();
  }
  let kept: String = text.chars().take(cols.saturating_sub(1)).collect();
  format!("{kept}…")
}

#[cfg(test)]
#[path = "tests/term_test.rs"]
mod tests;
