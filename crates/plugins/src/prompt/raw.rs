//! Raw mode for the length of one prompt: no line editing, no echo and no
//! signal keys, so each key arrives as it is pressed and Ctrl-C reaches the
//! picker as a byte. The saved attributes come back when the guard drops,
//! whatever the prompt's outcome. A SIGTERM or SIGHUP while a prompt is open
//! ends the process without that drop (shortcut: no signal handler; add one if
//! prompts ever run unattended).

use std::fs::File;
use std::io::{self, Read as _};
use std::os::fd::{AsFd, BorrowedFd};

use nix::sys::termios::{self, LocalFlags, SetArg, SpecialCharacterIndices, Termios, tcgetattr};

use crate::prompt::keys::KeyInput;

/// Whether `fd` is a terminal whose attributes can be read (and so changed).
pub(crate) fn probe(fd: BorrowedFd<'_>) -> bool {
  tcgetattr(fd).is_ok()
}

/// A terminal in raw mode until this drops.
pub(crate) struct Raw<'tty> {
  tty: &'tty File,
  saved: Termios,
  current: Termios,
}

impl<'tty> Raw<'tty> {
  /// Puts `tty` in raw mode: one byte per read, waiting for it.
  pub(crate) fn enter(tty: &'tty File) -> nix::Result<Raw<'tty>> {
    let saved = tcgetattr(tty.as_fd())?;
    let mut raw = saved.clone();
    raw
      .local_flags
      .remove(LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::ISIG);
    wait(&mut raw, 1, 0);
    termios::tcsetattr(tty.as_fd(), SetArg::TCSANOW, &raw)?;
    Ok(Raw {
      tty,
      saved,
      current: raw,
    })
  }

  /// One byte, after switching reads to wait for at least `min` bytes or
  /// `tenths` of a second.
  fn read(&mut self, min: u8, tenths: u8) -> io::Result<Option<u8>> {
    wait(&mut self.current, min, tenths);
    termios::tcsetattr(self.tty.as_fd(), SetArg::TCSANOW, &self.current)?;
    let mut byte = [0_u8; 1];
    let read = (&mut &*self.tty).read(&mut byte)?;
    Ok(byte.first().copied().filter(|_| read == 1))
  }
}

impl KeyInput for Raw<'_> {
  fn byte(&mut self) -> io::Result<Option<u8>> {
    self.read(1, 0)
  }

  fn byte_soon(&mut self) -> io::Result<Option<u8>> {
    let byte = self.read(0, 1);
    wait(&mut self.current, 1, 0);
    termios::tcsetattr(self.tty.as_fd(), SetArg::TCSANOW, &self.current)?;
    byte
  }
}

impl Drop for Raw<'_> {
  fn drop(&mut self) {
    // Nothing is left to report to if the terminal is gone.
    termios::tcsetattr(self.tty.as_fd(), SetArg::TCSANOW, &self.saved).ok();
  }
}

fn wait(termios: &mut Termios, min: u8, tenths: u8) {
  let chars = &mut termios.control_chars;
  if let Some(slot) = chars.get_mut(SpecialCharacterIndices::VMIN as usize) {
    *slot = min;
  }
  if let Some(slot) = chars.get_mut(SpecialCharacterIndices::VTIME as usize) {
    *slot = tenths;
  }
}

#[cfg(test)]
#[path = "tests/raw_test.rs"]
mod tests;
