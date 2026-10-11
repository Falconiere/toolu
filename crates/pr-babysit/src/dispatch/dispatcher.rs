//! The dispatcher's shared moves: commands it records (all of them, in a dry
//! run), state saves under the slot lock, the status document, and settling a
//! group.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde_json::{Map, Value, json};
use toolu_runtime::env::Env;
use toolu_state::time::iso_seconds;

use super::herdr::{argv, command};
use crate::error::{Failure, Result};
use crate::state::{load, open, write_json};

const GIT_TIMEOUT: Duration = Duration::from_secs(300);
const COOLDOWN: Duration = Duration::from_secs(3600);

/// What a dispatch runs against.
pub(crate) struct Options<'a> {
  /// The slot state.
  pub(crate) state_file: &'a Path,
  /// The environment of every command it runs.
  pub(crate) env: &'a Env,
  /// The `toolu` binary a fixer reports with.
  pub(crate) toolu: &'a Path,
  /// Record the commands; run and write nothing.
  pub(crate) dry: bool,
}

/// One `dispatch-fix` run over a slot.
pub(crate) struct Dispatcher<'a> {
  pub(super) file: &'a Path,
  pub(super) env: &'a Env,
  pub(super) toolu: &'a Path,
  pub(super) dry: bool,
  pub(super) commands: Vec<Vec<String>>,
  pub(super) state: Value,
}

/// Now, to the second.
pub(crate) fn now() -> String {
  iso_seconds(SystemTime::now())
}

/// The fixer's group `seq` in `state`.
pub(super) fn group_mut(state: &mut Value, seq: u64) -> Option<&mut Map<String, Value>> {
  let groups = state.pointer_mut("/fixer/groups")?.as_array_mut()?;
  let group = groups
    .iter_mut()
    .find(|group| group.get("seq").and_then(Value::as_u64) == Some(seq))?;
  group.as_object_mut()
}

/// Cools `host` until `until` for a provider limit.
fn cool(state: &mut Value, host: String, until: &str) {
  let Value::Object(root) = state else {
    return;
  };
  let cooldowns = root.entry("hostCooldowns").or_insert_with(|| json!({}));
  if let Value::Object(cooldowns) = cooldowns {
    cooldowns.insert(host, json!({"until": until, "reason": "host_limited"}));
  }
}

fn merge(target: &mut Map<String, Value>, patch: &Value) {
  if let Value::Object(patch) = patch {
    target.extend(
      patch
        .iter()
        .map(|(key, value)| (key.clone(), value.clone())),
    );
  }
}

impl<'a> Dispatcher<'a> {
  /// A dispatcher over the slot state `options` names.
  ///
  /// # Errors
  /// The state file's load failures.
  pub(crate) fn new(options: &Options<'a>) -> Result<Dispatcher<'a>> {
    Ok(Dispatcher {
      file: options.state_file,
      env: options.env,
      toolu: options.toolu,
      dry: options.dry,
      commands: Vec::new(),
      state: load(options.state_file)?,
    })
  }

  pub(super) fn record(&mut self, argv: &[String]) {
    self.commands.push(argv.to_vec());
  }

  /// Records and (unless dry) runs a git command; its trimmed stdout.
  pub(super) fn git(
    &mut self,
    argv: &[String],
    code: &'static str,
    message: String,
  ) -> Result<String> {
    self.record(argv);
    if self.dry {
      return Ok(String::new());
    }
    let ran = command(self.env, argv, GIT_TIMEOUT);
    if ran.status != 0 {
      return Err(Failure::new(code, message));
    }
    Ok(ran.output.trim().to_owned())
  }

  /// Runs a command whose failure is an answer, not an error.
  pub(super) fn probe(&self, argv: &[String]) -> bool {
    command(self.env, argv, GIT_TIMEOUT).status == 0
  }

  /// Applies `update` to the state under the slot lock (nothing in a dry run).
  pub(super) fn save(&mut self, update: impl FnOnce(&mut Value)) -> Result<()> {
    if self.dry {
      return Ok(());
    }
    let (_lock, mut state) = open(self.file, self.env)?;
    update(&mut state);
    write_json(self.file, &state, "state")?;
    self.state = state;
    Ok(())
  }

  pub(super) fn fixer(&self) -> Option<&Value> {
    self.state.get("fixer").filter(|fixer| fixer.is_object())
  }

  pub(super) fn fixer_field(&self, key: &str) -> Option<&Value> {
    self.fixer().and_then(|fixer| fixer.get(key))
  }

  pub(super) fn worktree_field(&self, key: &str) -> Option<&str> {
    self.state.get("herdrWorktree")?.get(key)?.as_str()
  }

  /// Fixer group `seq`.
  ///
  /// # Errors
  /// `state_malformed` when the state has no such group.
  pub(super) fn group(&self, seq: u64) -> Result<Value> {
    let groups = self.fixer_field("groups").and_then(Value::as_array);
    let found = groups.and_then(|groups| {
      groups
        .iter()
        .find(|group| group.get("seq").and_then(Value::as_u64) == Some(seq))
    });
    found.cloned().ok_or_else(|| {
      Failure::new("state_malformed", format!("fixer group {seq} is missing"))
        .with("source", "state")
    })
  }

  pub(super) fn patch_group(&mut self, seq: u64, patch: &Value) -> Result<()> {
    self.save(|state| {
      if let Some(group) = group_mut(state, seq) {
        merge(group, patch);
      }
    })
  }

  pub(super) fn patch_fixer(&mut self, patch: &Value) -> Result<()> {
    self.save(|state| {
      if let Some(Value::Object(fixer)) = state.get_mut("fixer") {
        merge(fixer, patch);
      }
    })
  }

  /// Settles group `seq`: done with its head, or failed with the reason (and,
  /// for a provider limit, an hour's cooldown on its host).
  pub(super) fn settle(&mut self, seq: u64, outcome: &str, head: &str, error: &str) -> Result<()> {
    let at = now();
    let until = iso_seconds(SystemTime::now() + COOLDOWN);
    self.save(|state| {
      let Some(group) = group_mut(state, seq) else {
        return;
      };
      if outcome == "done" {
        merge(
          group,
          &json!({"status": "done", "reason": null, "finishedAt": at, "head": head}),
        );
        return;
      }
      merge(
        group,
        &json!({"status": "failed", "reason": outcome, "finishedAt": at}),
      );
      if !error.is_empty() {
        group.insert("error".into(), json!(error));
      }
      let host = group
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
      if let Some(Value::Object(fixer)) = state.get_mut("fixer") {
        merge(fixer, &json!({"status": "failed", "reason": outcome}));
      }
      if outcome == "host_limited" {
        cool(state, host, &until);
      }
    })
  }

  /// After group `seq` of `count` settled: the next group, or the round done.
  /// False when the wait loop ends.
  pub(super) fn next(&mut self, seq: u64, count: u64) -> Result<bool> {
    if self.fixer_field("status").and_then(Value::as_str) != Some("running") {
      return Ok(false);
    }
    if seq >= count {
      self.patch_fixer(&json!({"status": "done", "finishedAt": now()}))?;
      return Ok(false);
    }
    self.patch_fixer(&json!({"current": seq + 1}))?;
    Ok(true)
  }

  /// The status document: the round, its groups, and the commits fixers made.
  pub(super) fn status(&self) -> Value {
    let path = self.worktree_field("path");
    let pr = self.worktree_field("prBranch").unwrap_or_default();
    let existing = path.filter(|path| Path::new(path).exists());
    let commits: Vec<String> = existing.map_or_else(Vec::new, |path| {
      let range = format!("refs/remotes/origin/{pr}..HEAD");
      let ran = command(
        self.env,
        &argv(&["git", "-C", path, "rev-list", "--reverse", &range]),
        GIT_TIMEOUT,
      );
      let listed = if ran.status == 0 {
        ran.output
      } else {
        String::new()
      };
      listed
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
    });
    let groups = self
      .fixer_field("groups")
      .and_then(Value::as_array)
      .map_or_else(Vec::new, |groups| groups.iter().map(summary).collect());
    let field = |key: &str| self.fixer_field(key).cloned().unwrap_or(Value::Null);
    json!({
      "version": 1,
      "status": self.fixer_field("status").cloned().unwrap_or_else(|| json!("none")),
      "reason": field("reason"),
      "group": field("current"),
      "worktree": path,
      "branch": self.worktree_field("branch"),
      "commits": commits,
      "groups": groups,
    })
  }
}

fn summary(group: &Value) -> Value {
  let mut out = Map::new();
  for key in [
    "seq", "tier", "host", "model", "effort", "agent", "status", "reason",
  ] {
    if let Some(value) = group.get(key) {
      out.insert(key.into(), value.clone());
    }
  }
  if let Some(error) = group
    .get("error")
    .filter(|error| error.as_str().is_some_and(|text| !text.is_empty()))
  {
    out.insert("error".into(), error.clone());
  }
  Value::Object(out)
}

#[cfg(test)]
#[path = "tests/dispatcher_test.rs"]
mod tests;
