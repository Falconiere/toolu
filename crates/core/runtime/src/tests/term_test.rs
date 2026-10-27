use super::{Style, Tone, intro, label_row, outro, paint, row, section, style_of, truncate};
use crate::env::Env;

#[test]
fn only_a_terminal_that_is_not_dumb_gets_rails_and_no_color_drops_only_color() {
  let none = Env::from_pairs([("TERM", "xterm-256color")]);
  assert_eq!(style_of(&none, false), Style::Plain);
  assert_eq!(style_of(&none, true), Style::Color);
  let dumb = Env::from_pairs([("TERM", "dumb")]);
  assert_eq!(style_of(&dumb, true), Style::Plain);
  let no_color = Env::from_pairs([("TERM", "xterm"), ("NO_COLOR", "1")]);
  assert_eq!(style_of(&no_color, true), Style::Mono);
  let empty = Env::from_pairs([("TERM", "xterm"), ("NO_COLOR", "")]);
  assert_eq!(style_of(&empty, true), Style::Color);
  assert!(!Style::Plain.rails());
  assert!(Style::Mono.rails() && Style::Color.rails());
}

#[test]
fn only_color_paints_and_every_tone_resets() {
  assert_eq!(paint(Style::Color, Tone::Good, "ok"), "\x1b[32mok\x1b[0m");
  assert_eq!(paint(Style::Color, Tone::Bad, "x"), "\x1b[31mx\x1b[0m");
  assert_eq!(paint(Style::Color, Tone::Dim, ""), "");
  assert_eq!(paint(Style::Mono, Tone::Good, "ok"), "ok");
  assert_eq!(paint(Style::Plain, Tone::Warn, "ok"), "ok");
}

#[test]
fn the_rails_builders_draw_exact_lines_without_escapes_under_mono() {
  let text = [
    intro(Style::Mono, "toolu plugins install"),
    section(Style::Mono, "claude"),
    row(Style::Mono, ("✔", Tone::Good), "jev", 8, "installed"),
    row(Style::Mono, ("●", Tone::Dim), "toolu", 8, ""),
    label_row(Style::Mono, "host", 14, "claude"),
    outro(Style::Mono, "1 installed"),
    outro(Style::Mono, ""),
  ]
  .concat();
  assert_eq!(
    text,
    "┌  toolu plugins install\n│\n◇  claude\n│  ✔ jev      installed\n│  ● toolu\n│  host          claude\n└  1 installed\n└\n"
  );
  assert!(!text.contains('\x1b'));
  assert!(row(Style::Color, ("✘", Tone::Bad), "jev", 3, "boom").contains("\x1b[31m✘\x1b[0m"));
}

#[test]
fn truncate_cuts_by_characters_and_marks_the_cut() {
  assert_eq!(truncate("abcdef", 6), "abcdef");
  assert_eq!(truncate("abcdef", 4), "abc…");
  assert_eq!(truncate("◼◼◼◼", 2), "◼…");
  assert_eq!(truncate("abc", 0), "…");
}
