//! Live progress at the terminal while host commands run: per host a bar that
//! advances per plugin and a spinner line naming the plugin at work. It writes
//! to `/dev/tty`, never stdout (the report), and clears itself before the report
//! prints. `begin`, `task`, `advance` and `end` draw at once; a thread only
//! turns the spinner. The cursor is never hidden, so an interrupted run leaves
//! the terminal usable.

use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use toolu_runtime::term::{Style, Tone, intro, paint, truncate};

/// The spinner's frames.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// How often the spinner turns.
const TICK: Duration = Duration::from_millis(80);
/// The bar's cells.
const CELLS: usize = 20;
/// The widest a host name or task may be drawn, so every line stays within 64
/// columns and never wraps (a wrapped line would break the redraw): the task
/// line is `│  ⠋ <task>…`, the spinner-only line `◇  <host>  ⠋ <task>…`.
const NAME_COLS: usize = 18;
/// See `NAME_COLS`.
const TASK_COLS: usize = 56;
/// See `NAME_COLS`.
const SPIN_TASK_COLS: usize = 38;

/// What the progress shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct State {
  /// The host at work.
  pub(crate) host: String,
  /// Its plugins; 0 when the count is not known (a spinner only).
  pub(crate) total: usize,
  /// The plugins done.
  pub(crate) done: usize,
  /// What is running now.
  pub(crate) task: String,
}

/// The two lines (one when `total` is 0) for `state` at spinner frame `tick`.
pub(crate) fn frame(state: &State, tick: usize, style: Style) -> String {
  let spinner = FRAMES.get(tick % FRAMES.len()).copied().unwrap_or("⠋");
  let host = truncate(&state.host, NAME_COLS);
  let task = if state.total == 0 {
    truncate(&state.task, SPIN_TASK_COLS)
  } else {
    truncate(&state.task, TASK_COLS)
  };
  let diamond = paint(style, Tone::Accent, "◇");
  let spin = paint(style, Tone::Accent, spinner);
  if state.total == 0 {
    return format!("{diamond}  {host}  {spin} {task}…");
  }
  let filled = (state.done.min(state.total) * CELLS) / state.total;
  let bar = format!(
    "{}{}",
    paint(style, Tone::Accent, &"█".repeat(filled)),
    paint(style, Tone::Dim, &"░".repeat(CELLS.saturating_sub(filled)))
  );
  format!(
    "{diamond}  {host}  {bar}  {}/{}\n{}  {spin} {task}…",
    state.done,
    state.total,
    paint(style, Tone::Dim, "│")
  )
}

/// What the drawing thread and the run share.
struct Shared {
  out: Mutex<Box<dyn Write + Send>>,
  draw: Mutex<Drawn>,
  running: AtomicBool,
  style: Style,
}

/// The state and what is on screen now.
#[derive(Default)]
struct Drawn {
  state: State,
  tick: usize,
  lines: usize,
}

impl Shared {
  fn write(&self, text: &str) {
    if let Ok(mut out) = self.out.lock() {
      // A terminal that went away has nowhere to show progress; the run goes on.
      out
        .write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .ok();
    }
  }

  /// Clears what is drawn, then draws `drawn` again unless `clear_only`.
  fn redraw(&self, drawn: &mut Drawn, clear_only: bool) {
    let mut text = match drawn.lines {
      0 => String::new(),
      1 => "\r\x1b[2K".to_owned(),
      _two => "\r\x1b[2K\x1b[1A\r\x1b[2K".to_owned(),
    };
    drawn.lines = 0;
    if !clear_only {
      text.push_str(&frame(&drawn.state, drawn.tick, self.style));
      drawn.lines = if drawn.state.total == 0 { 1 } else { 2 };
    }
    self.write(&text);
  }

  /// Turns the spinner until the run ends.
  fn turn(&self) {
    while self.running.load(Ordering::SeqCst) {
      std::thread::sleep(TICK);
      let Ok(mut drawn) = self.draw.lock() else {
        return;
      };
      if self.running.load(Ordering::SeqCst) {
        drawn.tick = drawn.tick.wrapping_add(1);
        self.redraw(&mut drawn, false);
      }
    }
  }

  fn change(&self, edit: impl FnOnce(&mut State)) {
    if let Ok(mut drawn) = self.draw.lock() {
      edit(&mut drawn.state);
      self.redraw(&mut drawn, false);
    }
  }
}

/// Progress for one run.
pub(crate) struct Progress {
  shared: Arc<Shared>,
  ticker: Mutex<Option<JoinHandle<()>>>,
}

impl fmt::Debug for Progress {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("Progress")
  }
}

impl Progress {
  /// Progress on `/dev/tty`; `None` for plain output (which `--json` forces),
  /// `--quiet`, or when there is no terminal to open.
  pub(crate) fn open(style: Style, quiet: bool) -> Option<Progress> {
    if !style.rails() || quiet {
      return None;
    }
    let tty = OpenOptions::new().write(true).open("/dev/tty").ok()?;
    Some(Progress::to_writer(Box::new(tty), style))
  }

  /// Progress written to `out`.
  pub(crate) fn to_writer(out: Box<dyn Write + Send>, style: Style) -> Progress {
    Progress {
      shared: Arc::new(Shared {
        out: Mutex::new(out),
        draw: Mutex::new(Drawn::default()),
        running: AtomicBool::new(false),
        style,
      }),
      ticker: Mutex::new(None),
    }
  }

  /// Draws the `┌  title` intro that opens the run.
  pub(crate) fn intro(&self, title: &str) {
    self.shared.write(&intro(self.shared.style, title));
  }

  /// Starts `host`'s bar over `total` plugins and the spinner, at `task`.
  pub(crate) fn begin(&self, host: &str, total: usize, task: &str) {
    self.shared.change(|state| {
      *state = State {
        host: host.to_owned(),
        total,
        done: 0,
        task: task.to_owned(),
      };
    });
    self.spin();
  }

  /// Names what runs now.
  pub(crate) fn task(&self, label: &str) {
    self
      .shared
      .change(|state| label.clone_into(&mut state.task));
  }

  /// One more plugin done.
  pub(crate) fn advance(&self) {
    self
      .shared
      .change(|state| state.done = (state.done + 1).min(state.total));
  }

  /// Stops the spinner and clears the lines.
  pub(crate) fn end(&self) {
    self.shared.running.store(false, Ordering::SeqCst);
    if let Ok(mut ticker) = self.ticker.lock()
      && let Some(handle) = ticker.take()
    {
      // A spinner thread that panicked has nothing left to stop.
      handle.join().ok();
    }
    if let Ok(mut drawn) = self.shared.draw.lock() {
      self.shared.redraw(&mut drawn, true);
    }
  }

  fn spin(&self) {
    if self.shared.running.swap(true, Ordering::SeqCst) {
      return;
    }
    let shared = Arc::clone(&self.shared);
    let handle = std::thread::Builder::new()
      .name("toolu-progress".to_owned())
      .spawn(move || shared.turn())
      .ok();
    if let Ok(mut ticker) = self.ticker.lock() {
      *ticker = handle;
    }
  }
}

impl Drop for Progress {
  fn drop(&mut self) {
    self.end();
  }
}

#[cfg(test)]
#[path = "tests/progress_test.rs"]
mod tests;
