//! Attempt-bound, replay-safe outcomes and cancellation receipts.

use super::{Store, Task, commit};

impl Store {
  /// Retain a recovery diagnostic without changing capacity or execution state.
  /// # Errors
  /// A stale attempt or durable storage failure.
  pub fn annotate(&self, key: &str, attempt: &str, note: &str) -> Result<Task, String> {
    self.change(key, attempt, |task| {
      task.note = Some(note.into());
      Ok(())
    })
  }
  /// Persist the verified reporting binary before launch intent.
  /// # Errors
  /// A stale attempt, invalid phase, changed executable or storage failure.
  pub fn prepared(&self, key: &str, attempt: &str, binary: &str) -> Result<Task, String> {
    self.change(key, attempt, |task| {
      if task.phase != "preparing"
        || task
          .worker_binary
          .as_deref()
          .is_some_and(|saved| saved != binary)
      {
        return Err("worker preparation identity changed".into());
      }
      task.worker_binary = Some(binary.into());
      Ok(())
    })
  }
  /// Retain a worker's explicit failure after its terminal has stopped.
  /// # Errors
  /// Stale attempt, unexpected terminal state, or durable storage failure.
  pub fn fail(&self, key: &str, attempt: &str, note: &str) -> Result<Task, String> {
    self.change(key, attempt, |task| {
      if task.phase == "failed" && task.note.as_deref() != Some(note) {
        return Err("failure conflicts with an existing receipt".into());
      }
      if !matches!(
        task.phase.as_str(),
        "preparing" | "running" | "held" | "failed"
      ) {
        return Err("task cannot accept a failure".into());
      }
      task.phase = "failed".into();
      task.note = Some(note.into());
      Ok(())
    })
  }
  /// Persist identity or uncertainty before acknowledging a launch operation.
  /// # Errors
  /// Stale attempt, invalid state, or storage failure.
  pub fn launched(
    &self,
    key: &str,
    attempt: &str,
    terminal: Option<&str>,
    note: &str,
  ) -> Result<Task, String> {
    self.change(key, attempt, |task| {
      if !matches!(task.phase.as_str(), "preparing" | "held" | "running") {
        return Err("attempt cannot be launched".into());
      }
      if task
        .terminal
        .as_deref()
        .is_some_and(|saved| Some(saved) != terminal)
      {
        return Err("terminal identity differs".into());
      }
      task.terminal = terminal.map(str::to_owned);
      task.launch_started = true;
      task.phase = if terminal.is_some() {
        "running"
      } else {
        "held"
      }
      .into();
      task.note = Some(note.into());
      Ok(())
    })
  }

  /// Accept a verified result; duplicate delivery must match commit and bytes.
  /// # Errors
  /// Stale/conflicting report, cancelled task or storage failure.
  pub fn report(
    &self,
    key: &str,
    attempt: &str,
    result: &str,
    bundle: &[u8],
  ) -> Result<Task, String> {
    if !commit(result) || bundle.is_empty() || bundle.len() > 512 * 1024 * 1024 {
      return Err("invalid result".into());
    }
    self.change(key, attempt, |task| {
      if task.phase == "done"
        && (task.result.as_deref() != Some(result) || self.output(key)? != bundle)
      {
        return Err("result conflicts with existing receipt".into());
      }
      if task.phase == "done" {
        return Ok(());
      }
      if !matches!(task.phase.as_str(), "running" | "held" | "preparing") {
        return Err("task cannot accept a result".into());
      }
      self.blob(key, "output", bundle)?;
      task.result = Some(result.into());
      task.phase = "done".into();
      Ok(())
    })
  }

  /// Record cancellation intent; an active worker keeps its capacity until stopped.
  /// # Errors
  /// Unknown task or durable write failure.
  pub fn cancel(&self, key: &str) -> Result<Task, String> {
    self.edit(|ledger| {
      let task = ledger.tasks.get_mut(key).ok_or("unknown task")?;
      task.phase = match task.phase.as_str() {
        "queued" | "cancelled" => "cancelled",
        "done" => "done",
        "failed" => "failed",
        _ => "cancelling",
      }
      .into();
      Ok(task.clone())
    })
  }

  /// Release cancellation capacity only after terminal shutdown is verified.
  /// # Errors
  /// Stale attempt, absent intent or storage failure.
  pub fn finish_cancel(&self, key: &str, attempt: &str) -> Result<Task, String> {
    self.change(key, attempt, |task| {
      if !matches!(task.phase.as_str(), "cancelling" | "cancelled") {
        return Err("missing cancellation intent".into());
      }
      task.phase = "cancelled".into();
      Ok(())
    })
  }

  /// Back off a failure known to precede launch, retaining the stable operation ID.
  /// # Errors
  /// An uncertain or running attempt cannot be rescheduled.
  pub fn retry(&self, key: &str, attempt: &str, now: u64, note: &str) -> Result<Task, String> {
    self.edit(|ledger| {
      let task = ledger.tasks.get_mut(key).ok_or("unknown task")?;
      if task.attempt.as_deref() != Some(attempt) {
        return Err("stale attempt".into());
      }
      if task.phase != "preparing" {
        return Err("attempt may still be running".into());
      }
      let machine = task.machine.take().ok_or("missing preparation worker")?;
      task.failures = task.failures.saturating_add(1);
      let delay = 1_u64 << task.failures.min(10);
      task.next_at = now.saturating_add(delay);
      task.phase = "queued".into();
      task.note = Some(note.into());
      let result = task.clone();
      let until = now.saturating_add(delay.saturating_mul(2));
      let cooldown = ledger.worker_cooldowns.entry(machine).or_default();
      *cooldown = (*cooldown).max(until);
      Ok(result)
    })
  }

  fn change(
    &self,
    key: &str,
    attempt: &str,
    change: impl FnOnce(&mut Task) -> Result<(), String>,
  ) -> Result<Task, String> {
    self.edit(|ledger| {
      let task = ledger.tasks.get_mut(key).ok_or("unknown task")?;
      if task.attempt.as_deref() != Some(attempt) {
        return Err("stale attempt".into());
      }
      change(task)?;
      Ok(task.clone())
    })
  }
}

#[cfg(test)]
#[path = "tests/transition_test.rs"]
mod tests;
