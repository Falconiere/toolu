//! Catch a panic inside `run_hook` as its message. A panic hook installed once
//! stays silent on a thread that is inside `run_hook`, so the host sees one toolu
//! line instead of Rust's report, and calls the previous hook on every other
//! thread. No hook is swapped back, so concurrent calls cannot race.

use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;

thread_local! {
  /// Whether this thread is inside [`catch`].
  static QUIET: Cell<bool> = const { Cell::new(false) };
}

static INSTALL: Once = Once::new();

/// Run `f`, or the message of the panic it raised.
pub(super) fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
  INSTALL.call_once(|| {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
      if !QUIET.with(Cell::get) {
        previous(info);
      }
    }));
  });
  QUIET.with(|quiet| quiet.set(true));
  let result = panic::catch_unwind(AssertUnwindSafe(f));
  QUIET.with(|quiet| quiet.set(false));
  result.map_err(|payload| message(payload.as_ref()))
}

fn message(payload: &(dyn Any + Send)) -> String {
  if let Some(text) = payload.downcast_ref::<&str>() {
    return (*text).to_owned();
  }
  if let Some(text) = payload.downcast_ref::<String>() {
    return text.clone();
  }
  "a panic with a non-text payload".to_owned()
}

#[cfg(test)]
#[path = "tests/panic_test.rs"]
mod tests;
