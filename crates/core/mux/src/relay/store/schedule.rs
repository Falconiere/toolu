//! Reserve capacity by ratio, with deterministic ties and serialization groups.

use super::{Store, Task, occupied};
use crate::relay::config::{Config, Machine};

impl Store {
  /// Reserve once, or leave the task queued while all eligible workers are full.
  /// # Errors
  /// Unknown task, corrupt ledger or durable write failure.
  pub fn reserve(&self, key: &str, config: &Config, now: u64) -> Result<Option<Task>, String> {
    self.edit(|ledger| {
      let task = ledger.tasks.get(key).ok_or("unknown task")?;
      if task.phase != "queued" || task.next_at > now {
        return Ok(None);
      }
      let active: Vec<_> = ledger.tasks.values().filter(|t| occupied(t)).collect();
      if task.request.group.as_ref().is_some_and(|group| {
        active
          .iter()
          .any(|t| t.request.group.as_ref() == Some(group))
      }) {
        return Ok(None);
      }
      let current = crate::relay::config::load(&self.root)?;
      let config = if self.root.join("config.json").exists() {
        &current
      } else {
        config
      };
      let selected = select(task, config, &active, &ledger.worker_cooldowns, now).cloned();
      let Some(worker) = selected else {
        return Ok(None);
      };
      let task = ledger.tasks.get_mut(key).ok_or("unknown task")?;
      task.phase = "preparing".into();
      task.machine = Some(worker.name.clone());
      task.worker = Some(worker);
      task.recipes = config
        .recipes
        .get(&task.request.origin)
        .cloned()
        .unwrap_or_default();
      task.attempt = Some(toolu_state::lock::token());
      task.terminal = None;
      task.launch_started = false;
      task.worker_binary = None;
      Ok(Some(task.clone()))
    })
  }
}

#[cfg(test)]
#[path = "tests/schedule_test.rs"]
mod tests;

fn select<'a>(
  task: &Task,
  config: &'a Config,
  active: &[&Task],
  cooldowns: &std::collections::BTreeMap<String, u64>,
  now: u64,
) -> Option<&'a Machine> {
  let occupied = |name: &str| -> u64 {
    active
      .iter()
      .filter(|t| t.machine.as_deref() == Some(name))
      .count() as u64
  };
  config
    .machines
    .values()
    .filter(|m| {
      !m.draining
        && cooldowns.get(&m.name).is_none_or(|until| *until <= now)
        && m.capacity > 0
        && m.agents.contains(&task.request.agent)
        && task
          .request
          .machine
          .as_ref()
          .is_none_or(|pin| pin == &m.name)
        && occupied(&m.name) < u64::from(m.capacity)
    })
    .min_by(|a, b| {
      (occupied(&a.name) * u64::from(b.capacity))
        .cmp(&(occupied(&b.name) * u64::from(a.capacity)))
        .then_with(|| a.name.cmp(&b.name))
    })
}
