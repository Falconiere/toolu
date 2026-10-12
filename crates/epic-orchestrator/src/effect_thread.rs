//! The effect thread: one call at a time, off the state thread, so status,
//! reports and `wait` stay live while a launch or a tracker wait runs. There
//! is no thread kill; each effect is bounded by its own sub-call deadlines.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::executor::{Call, Job};
use crate::socket_loop::Msg;

/// The running effect thread.
pub(crate) struct EffectThread {
  jobs: Option<Sender<(Call, Job)>>,
  handle: Option<JoinHandle<()>>,
}

impl EffectThread {
  /// Start the thread; each outcome goes to `done` as `Msg::Effect`.
  pub(crate) fn spawn(done: Sender<Msg>) -> Self {
    let (jobs, inbox) = mpsc::channel::<(Call, Job)>();
    let handle = thread::spawn(move || run(&inbox, &done));
    Self {
      jobs: Some(jobs),
      handle: Some(handle),
    }
  }

  /// Hand `job` to the thread.
  ///
  /// # Errors
  /// The thread is gone.
  pub(crate) fn send(&self, call: Call, job: Job) -> Result<(), ()> {
    self
      .jobs
      .as_ref()
      .ok_or(())?
      .send((call, job))
      .map_err(|_err| ())
  }

  /// Stop taking calls and wait for the running one to return.
  pub(crate) fn join(&mut self) {
    self.jobs = None;
    if let Some(handle) = self.handle.take() {
      let _joined = handle.join();
    }
  }
}

/// Run calls until the engine drops the thread or stops listening.
fn run(inbox: &Receiver<(Call, Job)>, done: &Sender<Msg>) {
  for (call, job) in inbox {
    let outcome = catch_unwind(AssertUnwindSafe(job))
      .unwrap_or_else(|_panic| "failed: effect panicked".to_owned());
    if done.send(Msg::Effect { call, outcome }).is_err() {
      return;
    }
  }
}

impl Drop for EffectThread {
  fn drop(&mut self) {
    self.join();
  }
}

#[cfg(test)]
#[path = "tests/effect_thread_test.rs"]
mod tests;
