//! Where engine effects run. `Inline` runs a call at once (tests, one-shot
//! verbs); `Thread` hands it to the effect thread, which holds at most one
//! call and returns its outcome to the state loop as `Msg::Effect`.

use crate::effect_thread::EffectThread;
use crate::model::Action;

/// One effect call: the issue (or epic) it belongs to, its action and token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Call {
  /// Issue or epic key.
  pub key: String,
  /// The action.
  pub action: Action,
  /// The pending action's token.
  pub token: String,
}

/// The body of one effect: it returns the outcome text.
pub(crate) type Job = Box<dyn FnOnce() -> String + Send>;

/// What submitting a call did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Submitted {
  /// It ran inline (or could not run); commit this outcome now.
  Done(String),
  /// The effect thread runs it; the outcome arrives later.
  Sent,
}

/// The outcome every call gets once the effect thread is gone.
pub(crate) const STOPPED: &str = "failed: effect thread stopped";

/// The engine's effect executor.
pub(crate) enum Executor {
  /// Run each call at once on the caller's thread.
  Inline,
  /// Send each call to the effect thread.
  Thread(EffectThread),
}

impl Executor {
  /// Run or send `job` for `call`.
  pub(crate) fn submit(&self, call: Call, job: Job) -> Submitted {
    match self {
      Self::Inline => Submitted::Done(job()),
      Self::Thread(thread) => match thread.send(call, job) {
        Ok(()) => Submitted::Sent,
        Err(()) => Submitted::Done(STOPPED.to_owned()),
      },
    }
  }
}

#[cfg(test)]
#[path = "tests/executor_test.rs"]
mod tests;
