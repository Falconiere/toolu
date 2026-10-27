use std::time::Duration;

use toolu_runtime::term::Style;

use super::{Progress, State, frame};
use crate::support::Screen;

#[test]
fn a_frame_shows_the_bar_the_count_and_the_task() {
  let state = State {
    host: "claude".to_owned(),
    total: 4,
    done: 1,
    task: "installing jev".to_owned(),
  };
  assert_eq!(
    frame(&state, 0, Style::Mono),
    "◇  claude  █████░░░░░░░░░░░░░░░  1/4\n│  ⠋ installing jev…"
  );
  let unknown = State { total: 0, ..state };
  assert_eq!(
    frame(&unknown, 1, Style::Mono),
    "◇  claude  ⠙ installing jev…"
  );
  assert!(frame(&unknown, 0, Style::Color).contains("\x1b[36m"));
}

#[test]
fn begin_task_and_advance_draw_at_once_and_end_clears_for_good() {
  let screen = Screen::default();
  let progress = Progress::to_writer(Box::new(screen.clone()), Style::Mono);
  progress.intro("toolu plugins install");
  progress.begin("claude", 3, "installing jev");
  progress.advance();
  let drawn = screen.text();
  assert!(
    drawn.starts_with("┌  toolu plugins install\n│\n"),
    "{drawn}"
  );
  assert!(
    drawn.contains("◇  claude  ██████░░░░░░░░░░░░░░  1/3"),
    "{drawn}"
  );
  assert!(drawn.contains("installing jev…"), "{drawn}");
  progress.end();
  let ended = screen.text();
  assert!(ended.ends_with("\r\x1b[2K\x1b[1A\r\x1b[2K"), "{ended:?}");
  std::thread::sleep(Duration::from_millis(200));
  assert_eq!(screen.text().len(), ended.len());
}

#[test]
fn the_spinner_turns_on_its_own_until_the_end() {
  let screen = Screen::default();
  let progress = Progress::to_writer(Box::new(screen.clone()), Style::Mono);
  progress.begin("codex", 0, "copying plugins");
  let first = screen.text().len();
  std::thread::sleep(Duration::from_millis(300));
  assert!(screen.text().len() > first, "the spinner never turned");
  drop(progress);
  assert!(screen.text().ends_with("\r\x1b[2K"));
}

#[test]
fn there_is_no_progress_for_plain_output_or_quiet() {
  assert!(Progress::open(Style::Plain, false).is_none());
  assert!(Progress::open(Style::Mono, true).is_none());
}

#[test]
fn no_line_of_a_frame_passes_64_columns_however_long_its_names() {
  let long = State {
    host: "h".repeat(40),
    total: 12,
    done: 3,
    task: "t".repeat(120),
  };
  let spinner_only = State {
    total: 0,
    ..long.clone()
  };
  for state in [long, spinner_only] {
    let drawn = frame(&state, 0, Style::Mono);
    for line in drawn.lines() {
      assert!(
        line.chars().count() <= 64,
        "{} columns: {line}",
        line.chars().count()
      );
    }
    assert!(drawn.contains('…'), "{drawn}");
  }
}
