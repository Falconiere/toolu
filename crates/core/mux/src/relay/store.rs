//! Transactional scheduling ledger. Terminal receipts are retained indefinitely.

use super::config::{Config, Machine};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod disk;
mod schedule;
mod transition;

/// Immutable caller input, including the caller-persisted deduplication key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submission {
  /// Caller-generated identity, persisted before delivery.
  pub key: String,
  /// Repository URL used with machine-local credentials.
  pub origin: String,
  /// Verified full input commit.
  pub base: String,
  /// Instructions delivered durably with the input.
  pub brief: String,
  /// Required installed agent CLI.
  pub agent: String,
  /// Agent invocation without shell evaluation.
  pub argv: Vec<String>,
  /// Optional explicit worker pin.
  pub machine: Option<String>,
  /// Tasks in this serialization group cannot overlap.
  pub group: Option<String>,
}

/// Durable state for one task; the operation identity survives attempts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
  /// Immutable payload.
  pub request: Submission,
  /// queued, preparing, running, held, cancelling, cancelled, or done.
  pub phase: String,
  /// Unique worker attempt, saved before any worker mutation.
  pub attempt: Option<String>,
  /// Reserved machine, retained while execution is uncertain.
  pub machine: Option<String>,
  /// Stable transport operation identity.
  pub operation: String,
  /// Verified Herdr address.
  pub terminal: Option<String>,
  /// Verified result commit.
  pub result: Option<String>,
  /// Persisted failure count.
  pub failures: u32,
  /// Earliest retry time, Unix seconds.
  pub next_at: u64,
  /// Last recovery or failure diagnostic.
  pub note: Option<String>,
  /// Immutable worker settings captured with the reservation.
  #[serde(default)]
  pub worker: Option<Machine>,
  /// Repository setup recipe captured with the reservation.
  #[serde(default)]
  pub recipes: Vec<Vec<String>>,
  /// Verified machine-local reporting executable selected before launch.
  #[serde(default)]
  pub worker_binary: Option<String>,
  /// Launch may have started; absence is only safe before its durable intent.
  #[serde(default)]
  pub launch_started: bool,
}

/// A relay-local ledger; all mutations hold the same strict lock.
#[derive(Debug, Clone)]
pub struct Store {
  root: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
  tasks: BTreeMap<String, Task>,
  #[serde(default)]
  worker_cooldowns: BTreeMap<String, u64>,
}

impl Store {
  /// Open a relay state directory without changing it.
  pub fn new(root: &Path) -> Self {
    Self {
      root: root.to_path_buf(),
    }
  }

  /// Accept only after the complete input and receipt are durable.
  /// # Errors
  /// Invalid input, conflicting retry or durable storage failure.
  pub fn submit(&self, request: Submission, bundle: &[u8]) -> Result<Task, String> {
    validate(&request, bundle)?;
    self.edit(|ledger| {
      if let Some(task) = ledger.tasks.get(&request.key) {
        return self.replay(task, &request, bundle);
      }
      let task = Task {
        request,
        phase: "queued".into(),
        attempt: None,
        machine: None,
        operation: toolu_state::lock::token(),
        terminal: None,
        result: None,
        failures: 0,
        next_at: 0,
        note: None,
        worker: None,
        recipes: Vec::new(),
        worker_binary: None,
        launch_started: false,
      };
      self.blob(&task.request.key, "input", bundle)?;
      ledger.tasks.insert(task.request.key.clone(), task.clone());
      Ok(task)
    })
  }

  fn replay(&self, task: &Task, request: &Submission, bundle: &[u8]) -> Result<Task, String> {
    if &task.request != request || self.input(&request.key)? != bundle {
      return Err("submission key conflicts with existing contents".into());
    }
    Ok(task.clone())
  }

  /// Read one receipt without inferring state from worker screens.
  /// # Errors
  /// Missing or unreadable receipt.
  pub fn load(&self, key: &str) -> Result<Task, String> {
    self
      .read()?
      .tasks
      .get(key)
      .cloned()
      .ok_or_else(|| "unknown task".into())
  }

  /// List all receipts, including archived terminal states.
  /// # Errors
  /// Corrupt or unreadable ledger.
  pub fn list(&self) -> Result<Vec<Task>, String> {
    Ok(self.read()?.tasks.into_values().collect())
  }

  /// Read tasks and worker preparation cooldowns from one ledger snapshot.
  /// # Errors
  /// The ledger cannot be read.
  pub fn status_snapshot(&self) -> Result<(Vec<Task>, BTreeMap<String, u64>), String> {
    let ledger = self.read()?;
    Ok((
      ledger.tasks.into_values().collect(),
      ledger.worker_cooldowns,
    ))
  }

  /// Return durable accepted input bytes.
  /// # Errors
  /// Invalid key or missing input.
  pub fn input(&self, key: &str) -> Result<Vec<u8>, String> {
    self.read_blob(key, "input")
  }

  /// Return durable verified result bytes.
  /// # Errors
  /// The task has no accepted result or storage is unreadable.
  pub fn output(&self, key: &str) -> Result<Vec<u8>, String> {
    if self.load(key)?.phase != "done" {
      return Err("task is not done".into());
    }
    self.read_blob(key, "output")
  }

  /// Read one bounded result segment without repeatedly loading the entire bundle.
  /// # Errors
  /// Missing result, invalid offset, changed size or storage failure.
  pub fn output_chunk(
    &self,
    key: &str,
    offset: u64,
    limit: usize,
  ) -> Result<(u64, Vec<u8>), String> {
    use std::io::{Read as _, Seek as _};
    if self.load(key)?.phase != "done" || !safe(key) || limit > 256 * 1024 {
      return Err("invalid result read".into());
    }
    let mut file = std::fs::File::open(self.root.join("tasks").join(key).join("output"))
      .map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if offset > size {
      return Err("invalid offset".into());
    }
    file
      .seek(std::io::SeekFrom::Start(offset))
      .map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(limit);
    file
      .take(limit as u64)
      .read_to_end(&mut bytes)
      .map_err(|e| e.to_string())?;
    Ok((size, bytes))
  }

  /// Serialize configuration changes against scheduling reservations.
  /// # Errors
  /// Storage, active assignment, or configuration validation failure.
  pub fn configure(
    &self,
    change: impl FnOnce(&[Task]) -> Result<Config, String>,
  ) -> Result<Config, String> {
    self.edit(|ledger| change(&ledger.tasks.values().cloned().collect::<Vec<_>>()))
  }
}

fn validate(request: &Submission, bundle: &[u8]) -> Result<(), String> {
  if !safe(&request.key)
    || !commit(&request.base)
    || bundle.is_empty()
    || bundle.len() > 512 * 1024 * 1024
    || request.brief.len() > 1024 * 1024
    || request.origin.is_empty()
    || request.origin.starts_with('-')
    || request.origin.chars().any(char::is_control)
    || !matches!(
      request.agent.as_str(),
      "claude" | "codex" | "muse" | "cursor" | "opencode"
    )
    || request.argv.first().is_none_or(String::is_empty)
    || request.machine.as_deref().is_some_and(|m| !safe(m))
    || request
      .group
      .as_ref()
      .is_some_and(|g| g.is_empty() || g.len() > 512)
  {
    return Err("invalid submission".into());
  }
  Ok(())
}

pub(super) fn safe(value: &str) -> bool {
  !value.is_empty()
    && value.len() <= 128
    && value
      .bytes()
      .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn commit(value: &str) -> bool {
  matches!(value.len(), 40 | 64)
    && value
      .bytes()
      .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Terminal states alone release reservations.
pub fn occupied(task: &Task) -> bool {
  matches!(
    task.phase.as_str(),
    "preparing" | "running" | "held" | "cancelling"
  )
}

#[cfg(test)]
#[path = "tests/store_test.rs"]
mod tests;
