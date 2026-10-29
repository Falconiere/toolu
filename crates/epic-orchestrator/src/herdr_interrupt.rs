//! Wake the herdr subscription thread without a short timer (#451): the live
//! stream is shared here, so a stop or a resubscribe shuts it down and its
//! blocking read returns at once, and a pause waits on a condition variable
//! that the same trigger signals.

use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// How long a quiet herdr read or a pause between sessions waits.
pub(crate) const IDLE: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
struct Slot {
  stream: Option<UnixStream>,
  process: Option<u32>,
  pending: bool,
}

/// The herdr thread's wake-up: a stream to shut down and a pause to end.
#[derive(Debug, Default)]
pub(crate) struct Interrupt {
  slot: Mutex<Slot>,
  signal: Condvar,
}

impl Interrupt {
  /// Share a handle to the live `stream`; an earlier trigger shuts it at once.
  pub(crate) fn publish(&self, stream: &UnixStream) {
    let Ok(mut slot) = self.slot.lock() else {
      return;
    };
    let Ok(handle) = stream.try_clone() else {
      return;
    };
    if slot.pending {
      let _closed = handle.shutdown(Shutdown::Both);
    }
    slot.stream = Some(handle);
  }

  /// Forget the stream once its session ended.
  pub(crate) fn clear(&self) {
    if let Ok(mut slot) = self.slot.lock() {
      slot.stream = None;
    }
  }

  /// Register the hc stream process group for cancellation.
  pub(crate) fn publish_process(&self, pid: u32) {
    if let Ok(mut slot) = self.slot.lock() {
      if slot.pending {
        let _stopped = toolu_runtime::process::group::terminate(pid);
        return;
      }
      slot.process = Some(pid);
    }
  }

  /// Forget the hc process after it is reaped.
  pub(crate) fn clear_process(&self) {
    if let Ok(mut slot) = self.slot.lock() {
      slot.process = None;
    }
  }

  /// Stop only the hc stream so a failed snapshot can reconnect.
  pub(crate) fn end_process(&self) {
    if let Ok(mut slot) = self.slot.lock()
      && let Some(pid) = slot.process.take()
    {
      let _stopped = toolu_runtime::process::group::terminate(pid);
    }
  }

  /// End the current read and any pause: stop or resubscribe.
  pub(crate) fn trigger(&self) {
    let Ok(mut slot) = self.slot.lock() else {
      return;
    };
    slot.pending = true;
    if let Some(stream) = slot.stream.take() {
      let _closed = stream.shutdown(Shutdown::Both);
    }
    if let Some(pid) = slot.process.take() {
      let _stopped = toolu_runtime::process::group::terminate(pid);
    }
    self.signal.notify_all();
  }

  /// Wait up to `limit`, or until a trigger; consume the trigger.
  pub(crate) fn pause(&self, limit: Duration) {
    let Ok(slot) = self.slot.lock() else {
      return;
    };
    let waited = self
      .signal
      .wait_timeout_while(slot, limit, |slot| !slot.pending);
    if let Ok((mut slot, _timeout)) = waited {
      slot.pending = false;
    }
  }

  /// Consume a trigger that arrived before a new session started.
  pub(crate) fn reset(&self) {
    if let Ok(mut slot) = self.slot.lock() {
      slot.pending = false;
    }
  }
}

#[cfg(test)]
#[path = "tests/herdr_interrupt_test.rs"]
mod tests;
