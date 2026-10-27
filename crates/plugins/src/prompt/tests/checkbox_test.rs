use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nix::pty::openpty;
use nix::sys::termios::tcgetattr;
use toolu_runtime::term::Style;

use super::{Checkbox, Done, Pick, collapsed, render};
use crate::fail::Res;
use crate::prompt::keys::Key;
use crate::prompt::{Ask, Choice};

fn options() -> Vec<Choice> {
  vec![
    ("claude".to_owned(), Some("Claude Code".to_owned())),
    ("codex".to_owned(), Some("x".repeat(200))),
    ("muse".to_owned(), None),
  ]
}

#[test]
fn keys_move_toggle_and_confirm_and_an_empty_choice_is_refused() {
  let mut pick = Pick::new(3, true);
  let pressed: Vec<Option<Done>> = [Key::Up, Key::Down, Key::Down, Key::Toggle]
    .into_iter()
    .map(|key| pick.press(key))
    .collect();
  assert!(pressed.iter().all(Option::is_none));
  assert_eq!(
    (pick.cursor, pick.chosen.clone()),
    (1, vec![true, false, true])
  );
  pick.press(Key::All);
  assert_eq!(pick.chosen, [true, true, true]);
  pick.press(Key::All);
  assert_eq!((pick.press(Key::Enter), pick.empty), (None, true));
  pick.press(Key::Toggle);
  assert_eq!(pick.press(Key::Enter), Some(Done::Chose(vec![1])));
}

#[test]
fn a_single_choice_takes_the_cursor_row_and_ignores_toggles() {
  let mut one = Pick::new(3, false);
  assert_eq!(one.press(Key::Toggle), None);
  assert_eq!(one.press(Key::Down), None);
  assert_eq!(one.press(Key::Enter), Some(Done::Chose(vec![1])));
  assert_eq!(one.press(Key::Cancel), Some(Done::Cancelled));
}

#[test]
fn the_open_list_and_its_collapsed_answer_draw_on_rails() {
  let mut pick = Pick::new(3, true);
  pick.press(Key::Down);
  pick.press(Key::Toggle);
  let shown = render(Style::Mono, "Which hosts?", &options(), &pick);
  let lines: Vec<&str> = shown.lines().collect();
  assert_eq!(lines.first(), Some(&"◆  Which hosts?"));
  assert_eq!(lines.get(1), Some(&"│    ◼ claude  Claude Code"));
  assert!(
    lines
      .get(2)
      .is_some_and(|line| line.starts_with("│  > ◻ codex   xxx") && line.ends_with('…'))
  );
  assert!(
    lines.iter().all(|line| line.chars().count() <= 76),
    "{shown}"
  );
  assert_eq!(lines.get(3), Some(&"│    ◼ muse"));
  assert_eq!(
    lines.get(4),
    Some(&"└  ↑/↓ move · space toggle · a all · enter")
  );
  assert!(!shown.contains('\x1b'));
  assert_eq!(
    collapsed(Style::Mono, "Which hosts?", "claude, muse"),
    "◇  Which hosts?  claude, muse\n│\n"
  );
}

/// What a person typing `keys` (each after a pause) into a real pty sees and
/// gets; the pty's attributes must be the same before and after the prompt.
fn on_pty(keys: &[&'static [u8]], many: bool) -> (Res<Vec<usize>>, String) {
  let pty = openpty(None, None).expect("a pty");
  let tty = File::from(pty.slave);
  let master = File::from(pty.master);
  let screen = Arc::new(Mutex::new(Vec::new()));
  let (mut reader, sink) = (master.try_clone().expect("clone"), Arc::clone(&screen));
  // The reader drains the pty so the picker's drawing never blocks; it ends with the test process.
  std::thread::spawn(move || {
    let mut chunk = [0_u8; 1024];
    while let Ok(read @ 1..) = reader.read(&mut chunk) {
      sink
        .lock()
        .expect("screen")
        .extend_from_slice(chunk.get(..read).unwrap_or_default());
    }
  });
  let keys: Vec<&'static [u8]> = keys.to_vec();
  let mut typist = master;
  let typing = std::thread::spawn(move || {
    for key in keys {
      std::thread::sleep(Duration::from_millis(150));
      typist.write_all(key).expect("type");
    }
  });
  let before = tcgetattr(tty.as_fd()).expect("attributes");
  let mut checkbox = Checkbox::new(tty.try_clone().expect("clone"), Style::Mono);
  assert_eq!(
    tcgetattr(tty.as_fd()).expect("attributes").local_flags,
    before.local_flags
  );
  let result = if many {
    checkbox.many("Which hosts?", &options())
  } else {
    checkbox.one("Which host?", &options()).map(|at| vec![at])
  };
  let after = tcgetattr(tty.as_fd()).expect("attributes");
  assert_eq!(settled(after.local_flags), settled(before.local_flags));
  assert_eq!(after.control_chars, before.control_chars);
  typing.join().expect("typing");
  std::thread::sleep(Duration::from_millis(100));
  let shown = String::from_utf8_lossy(&screen.lock().expect("screen")).into_owned();
  (result, shown)
}

#[test]
fn a_real_terminal_picks_with_arrows_and_space_and_collapses_the_answer() {
  let (result, shown) = on_pty(&[b"\x1b[B", b" ", b"\r"], true);
  assert_eq!(result, Ok(vec![0, 2]));
  assert!(shown.contains("◆  Which hosts?"), "{shown}");
  assert!(shown.contains("◇  Which hosts?  claude, muse"), "{shown}");
}

#[test]
fn a_real_terminal_refuses_an_empty_choice_then_takes_all() {
  let (result, shown) = on_pty(&[b"a", b"\r", b"a", b"\r"], true);
  assert_eq!(result, Ok(vec![0, 1, 2]));
  assert!(shown.contains("choose at least one"), "{shown}");
}

#[test]
fn q_ctrl_c_and_a_lone_escape_cancel_on_a_real_terminal() {
  for key in [&b"q"[..], b"\x03", b"\x1b"] {
    let (result, shown) = on_pty(&[key], true);
    assert_eq!(
      result.map_err(|fail| fail.message),
      Err("cancelled".to_owned()),
      "{key:?}"
    );
    assert!(shown.contains("◇  Which hosts?  cancelled"), "{shown}");
  }
}

#[test]
fn one_takes_the_row_under_the_cursor() {
  let (result, _) = on_pty(&[b"\x1b[B", b"\r"], false);
  assert_eq!(result, Ok(vec![1]));
}

/// Attributes as the program set them: BSD kernels raise `PENDIN`, a status
/// bit, themselves when canonical mode resumes.
fn settled(flags: nix::sys::termios::LocalFlags) -> nix::sys::termios::LocalFlags {
  flags - nix::sys::termios::LocalFlags::PENDIN
}
