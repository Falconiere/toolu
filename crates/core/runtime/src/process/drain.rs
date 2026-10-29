//! Stream drains: each child stream is read to its end on a thread, keeping the
//! bytes the shared budget allows and dropping the rest, so a chatty child
//! never blocks on a full pipe.

use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::ChunkObserver;

const POLL: Duration = Duration::from_millis(10);

/// The bytes still to keep across both streams, and whether any were dropped.
#[derive(Debug, Clone)]
pub(super) struct Budget(Arc<Mutex<(usize, bool)>>);

impl Budget {
  /// A budget of `bytes`.
  pub(super) fn new(bytes: usize) -> Budget {
    Budget(Arc::new(Mutex::new((bytes, false))))
  }

  /// How many of `offered` bytes to keep; marks the output truncated when not all.
  fn take(&self, offered: usize) -> usize {
    let mut state = lock(&self.0);
    let kept = offered.min(state.0);
    state.0 -= kept;
    state.1 |= kept < offered;
    kept
  }

  /// Whether any byte was dropped.
  pub(super) fn truncated(&self) -> bool {
    lock(&self.0).1
  }
}

/// A stream being read on its own thread.
#[derive(Debug)]
pub(super) struct Drain {
  bytes: Arc<Mutex<Vec<u8>>>,
  reader: Option<JoinHandle<()>>,
}

impl Drain {
  /// Starts reading `stream`; a missing stream is an empty, finished drain.
  pub(super) fn start<R: Read + Send + 'static>(
    stream: Option<R>,
    budget: &Budget,
    observer: Option<ChunkObserver>,
    cancel: Arc<AtomicBool>,
  ) -> Drain {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let reader = stream.map(|stream| {
      let (bytes, budget) = (Arc::clone(&bytes), budget.clone());
      std::thread::spawn(move || read_all(stream, &bytes, &budget, observer.as_ref(), &cancel))
    });
    Drain { bytes, reader }
  }

  /// Whether the stream reached its end (or failed).
  pub(super) fn finished(&self) -> bool {
    self.reader.as_ref().is_none_or(JoinHandle::is_finished)
  }

  /// Wait for a canceled reader using the shared cleanup deadline.
  pub(super) fn finish_by(&mut self, deadline: Instant) {
    // shortcut: a blocked observer callback cannot be interrupted; use a cancellable observer contract if that changes.
    while !self.finished() && Instant::now() < deadline {
      std::thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
    if self.finished()
      && let Some(reader) = self.reader.take()
    {
      let _finished = reader.join();
    }
  }

  /// The bytes kept so far, decoded lossily.
  pub(super) fn text(&self) -> String {
    String::from_utf8_lossy(&lock(&self.bytes)).into_owned()
  }

  /// The bytes kept so far, exactly as the child wrote them.
  pub(super) fn bytes(&self) -> Vec<u8> {
    lock(&self.bytes).clone()
  }
}

fn read_all<R: Read>(
  mut stream: R,
  bytes: &Mutex<Vec<u8>>,
  budget: &Budget,
  observer: Option<&ChunkObserver>,
  cancel: &AtomicBool,
) {
  let mut chunk = [0_u8; 8192];
  loop {
    if cancel.load(Ordering::Acquire) {
      return;
    }
    match stream.read(&mut chunk) {
      Ok(0) => return,
      Ok(read) => {
        if let Some(observer) = observer {
          observer(chunk.get(..read).unwrap_or(&chunk));
        }
        let kept = budget.take(read);
        lock(bytes).extend(chunk.iter().take(kept));
      }
      Err(err) if err.kind() == ErrorKind::Interrupted => {}
      Err(err) if err.kind() == ErrorKind::WouldBlock => std::thread::sleep(POLL),
      Err(_) => return,
    }
  }
}

/// A poisoned lock still holds the bytes gathered before the panic.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "tests/drain_test.rs"]
mod tests;
