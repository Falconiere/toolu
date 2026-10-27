use std::fs::File;
use std::io::Write as _;
use std::os::fd::AsFd;

use nix::pty::openpty;
use nix::sys::termios::{LocalFlags, tcgetattr};

use super::{Raw, probe};
use crate::prompt::keys::KeyInput;

#[test]
fn raw_mode_holds_only_while_the_guard_lives_and_reads_bytes_as_typed() {
  let pty = openpty(None, None).expect("a pty");
  let tty = File::from(pty.slave);
  let mut master = File::from(pty.master);
  assert!(probe(tty.as_fd()));
  let before = tcgetattr(tty.as_fd()).expect("attributes");
  {
    let mut raw = Raw::enter(&tty).expect("raw mode");
    let now = tcgetattr(tty.as_fd()).expect("attributes");
    assert!(!now.local_flags.contains(LocalFlags::ICANON));
    assert!(!now.local_flags.contains(LocalFlags::ECHO));
    assert!(!now.local_flags.contains(LocalFlags::ISIG));
    master.write_all(b"x").expect("type");
    assert_eq!(raw.byte().expect("read"), Some(b'x'));
    assert_eq!(raw.byte_soon().expect("read"), None);
  }
  let after = tcgetattr(tty.as_fd()).expect("attributes");
  assert_eq!(settled(after.local_flags), settled(before.local_flags));
  assert_eq!(after.control_chars, before.control_chars);
}

#[test]
fn a_file_that_is_not_a_terminal_cannot_go_raw() {
  let file = tempfile::tempfile().expect("a file");
  assert!(!probe(file.as_fd()));
  assert!(Raw::enter(&file).is_err());
}

/// Attributes as the program set them: BSD kernels raise `PENDIN`, a status
/// bit, themselves when canonical mode resumes.
fn settled(flags: nix::sys::termios::LocalFlags) -> nix::sys::termios::LocalFlags {
  flags - nix::sys::termios::LocalFlags::PENDIN
}
