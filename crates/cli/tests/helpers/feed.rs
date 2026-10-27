//! Feed a hook its payload as a host does. A command that never reads stdin
//! (`hook register`, an early exit) may close the pipe before the write lands,
//! and on a loaded machine it often does: that `BrokenPipe` is not a failure.

use std::io::{ErrorKind, Write as _};
use std::process::ChildStdin;

/// Write `payload` to the child's piped stdin and close it.
pub(crate) fn feed(stdin: Option<ChildStdin>, payload: &[u8]) -> std::io::Result<()> {
  let mut stdin = stdin.ok_or_else(|| std::io::Error::other("piped stdin missing"))?;
  match stdin.write_all(payload) {
    Err(err) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
    other => other,
  }
}
