//! One engine effect's sleep budget, shared by every client the effect builds
//! on its thread, so all of its retries together stay within
//! `EPIC_API_MAX_SLEEP_S`.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use super::{Budget, Policy};

thread_local! {
  static SCOPE: RefCell<Option<Arc<Mutex<Budget>>>> = const { RefCell::new(None) };
}

/// Clears the scope even when the effect unwinds.
struct Reset;

impl Drop for Reset {
  fn drop(&mut self) {
    SCOPE.with(|scope| scope.replace(None));
  }
}

/// Run `work` as one engine effect under `policy`'s total sleep budget.
pub(crate) fn effect_scope<T>(policy: &Policy, work: impl FnOnce() -> T) -> T {
  let budget = Arc::new(Mutex::new(policy.effect()));
  SCOPE.with(|scope| scope.replace(Some(budget)));
  let _reset = Reset;
  work()
}

/// The running effect's budget, or a fresh one limited only by the per-wait
/// cap (one CLI call).
pub(crate) fn scoped_budget() -> Arc<Mutex<Budget>> {
  SCOPE
    .with(|scope| scope.borrow().clone())
    .unwrap_or_else(|| Arc::new(Mutex::new(Budget::unbounded())))
}

#[cfg(test)]
#[path = "tests/scope_test.rs"]
mod tests;
