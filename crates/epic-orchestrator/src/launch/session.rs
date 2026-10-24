//! Capture a host's native session id for a worktree (`session.ts`); never
//! substitute the host's unrelated last session. More than one candidate is
//! no answer.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::RegexBuilder;
use serde_json::Value;
use toolu_runtime::env::Env;

use super::digest::md5_hex;
use super::failure::Failure;
use crate::hosts::HostKind;

pub(super) const MAX_FILES: usize = 256;
const MAX_BYTES: usize = 65_536;
const MAX_RECORDS: usize = 32;
const DAY_MS: u64 = 86_400_000;

pub(super) fn valid_id(value: &str) -> bool {
  RegexBuilder::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
    .case_insensitive(true)
    .build()
    .is_ok_and(|re| re.is_match(value))
}

pub(super) fn vanished(err: &std::io::Error) -> bool {
  err.kind() == std::io::ErrorKind::NotFound
}

/// The first 64 KiB of `path` as text, never reading past them; `None` once
/// it is gone.
fn head(path: &Path) -> Result<Option<String>, String> {
  let mut bytes = Vec::new();
  let read = std::fs::File::open(path).and_then(|file| {
    let limit = u64::try_from(MAX_BYTES).unwrap_or(u64::MAX);
    file.take(limit).read_to_end(&mut bytes)
  });
  match read {
    Ok(_) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
    Err(err) if vanished(&err) => Ok(None),
    Err(err) => Err(format!("{}: {err}", path.display())),
  }
}

pub(super) fn one(found: BTreeSet<String>) -> Option<String> {
  (found.len() == 1)
    .then(|| found.into_iter().next())
    .flatten()
}

fn epoch_ms(value: Option<&Value>) -> Option<f64> {
  value.and_then(Value::as_f64).filter(|n| n.is_finite())
}

/// Cursor Agent keeps each chat at `<config>/chats/<md5(cwd)>/<id>/meta.json`.
fn cursor(worktree: &str, since: f64, env: &Env) -> Result<Option<String>, String> {
  let config = env
    .get("CURSOR_CONFIG_DIR")
    .filter(|dir| !dir.trim().is_empty())
    .map(PathBuf::from)
    .or_else(|| {
      env
        .get("XDG_CONFIG_HOME")
        .filter(|dir| !dir.trim().is_empty())
        .map(|dir| Path::new(dir).join("cursor"))
    })
    .unwrap_or_else(|| env.home().join(".cursor"));
  let root = config.join("chats").join(md5_hex(worktree.as_bytes()));
  let chats: Vec<String> = match std::fs::read_dir(&root) {
    Ok(entries) => entries
      .filter_map(Result::ok)
      .filter_map(|e| e.file_name().to_str().map(str::to_owned))
      .collect(),
    Err(err) if vanished(&err) => return Ok(None),
    Err(err) => return Err(format!("{}: {err}", root.display())),
  };
  if chats.len() > MAX_FILES {
    return Ok(None);
  }
  let mut found = BTreeSet::new();
  for chat in chats.into_iter().filter(|chat| valid_id(chat)) {
    let Some(text) = head(&root.join(&chat).join("meta.json"))? else {
      continue;
    };
    let Ok(meta) = serde_json::from_str::<Value>(&text) else {
      continue;
    };
    let fresh = epoch_ms(meta.get("createdAtMs")).is_some_and(|created| created >= since);
    if fresh && meta.get("cwd").and_then(Value::as_str) == Some(worktree) {
      found.insert(chat);
    }
  }
  Ok(one(found))
}

/// Every `*.jsonl` under `root`, or `None` past [`MAX_FILES`].
fn jsonl_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<bool, String> {
  let entries = match std::fs::read_dir(root) {
    Ok(entries) => entries,
    Err(err) if vanished(&err) => return Ok(true),
    Err(err) => return Err(format!("{}: {err}", root.display())),
  };
  for entry in entries.filter_map(Result::ok) {
    let path = entry.path();
    if path.is_dir() {
      if !jsonl_files(&path, out)? {
        return Ok(false);
      }
    } else if path.extension().is_some_and(|ext| ext == "jsonl") {
      out.push(path);
      if out.len() > MAX_FILES {
        return Ok(false);
      }
    }
  }
  Ok(true)
}

/// The `sessions/<yyyy>/<mm>/<dd>` folders that can hold a session started at
/// any of `instants` (epoch ms): hosts name the folder by their local date,
/// which is always the UTC date one day either side or the same.
pub(super) fn day_dirs(base: &Path, instants: &[u64]) -> BTreeSet<PathBuf> {
  instants
    .iter()
    .flat_map(|&at| [at.saturating_sub(DAY_MS), at, at.saturating_add(DAY_MS)])
    .map(|at| day_dir(base, at))
    .collect()
}

fn day_dir(base: &Path, epoch_ms: u64) -> PathBuf {
  let iso = toolu_state::time::iso_seconds(UNIX_EPOCH + Duration::from_millis(epoch_ms));
  iso
    .get(..10)
    .unwrap_or("")
    .split('-')
    .fold(base.join("sessions"), |dir, part| dir.join(part))
}

/// The head of `path` when it was written at or after `since` (epoch ms);
/// `None` when older or gone.
pub(super) fn fresh_head(path: &Path, since: u64) -> Result<Option<String>, String> {
  let since_time = UNIX_EPOCH + Duration::from_millis(since);
  let fresh = match std::fs::metadata(path).and_then(|meta| meta.modified()) {
    Ok(modified) => modified >= since_time,
    Err(err) if vanished(&err) => false,
    Err(err) => return Err(format!("{}: {err}", path.display())),
  };
  Ok(fresh.then(|| head(path)).transpose()?.flatten())
}

/// The first [`MAX_RECORDS`] JSON lines of `text`.
pub(super) fn records(text: &str) -> impl Iterator<Item = Value> + '_ {
  text
    .split('\n')
    .take(MAX_RECORDS)
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
}

fn session_of(kind: HostKind, entry: &Value, worktree: &str) -> Option<String> {
  let (cwd, id) = match kind {
    HostKind::Codex if entry.get("type").and_then(Value::as_str) == Some("session_meta") => {
      let payload = entry.get("payload")?;
      (payload.get("cwd"), payload.get("id"))
    }
    HostKind::Claude => (entry.get("cwd"), entry.get("sessionId")),
    HostKind::Codex | HostKind::Cursor | HostKind::Muse => return None,
  };
  let id = id?.as_str()?;
  (cwd?.as_str()? == worktree && valid_id(id)).then(|| id.to_owned())
}

fn transcripts(
  kind: HostKind,
  worktree: &str,
  since: u64,
  env: &Env,
) -> Result<Option<String>, String> {
  let roots: BTreeSet<PathBuf> = if kind == HostKind::Codex {
    let home = env
      .get("CODEX_HOME")
      .map_or_else(|| env.home().join(".codex"), PathBuf::from);
    let now = toolu_state::time::epoch_millis(SystemTime::now());
    day_dirs(&home, &[since, now])
  } else {
    let home = env
      .get("CLAUDE_CONFIG_DIR")
      .map_or_else(|| env.home().join(".claude"), PathBuf::from);
    let slug: String = worktree
      .chars()
      .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
      .collect();
    BTreeSet::from([home.join("projects").join(slug)])
  };
  let mut files = Vec::new();
  for root in &roots {
    if !jsonl_files(root, &mut files)? {
      return Ok(None);
    }
  }
  let mut found = BTreeSet::new();
  for path in files {
    let Some(text) = fresh_head(&path, since)? else {
      continue;
    };
    found.extend(records(&text).filter_map(|entry| session_of(kind, &entry, worktree)));
  }
  Ok(one(found))
}

/// The one session `kind` recorded for `worktree` since `since` (epoch ms).
///
/// # Errors
/// A session store cannot be read for a reason other than a file vanishing.
pub(crate) fn capture_session(
  kind: HostKind,
  worktree: &str,
  since: u64,
  env: &Env,
) -> Result<Option<String>, String> {
  // Hosts write epoch milliseconds as JSON numbers; compare as they do.
  let since_f = f64::from(u32::try_from(since >> 32).unwrap_or(u32::MAX)) * 4_294_967_296.0
    + f64::from(u32::try_from(since & 0xffff_ffff).unwrap_or(0));
  match kind {
    HostKind::Cursor => cursor(worktree, since_f, env),
    HostKind::Muse => super::muse_session::capture(worktree, since, env),
    HostKind::Claude | HostKind::Codex => transcripts(kind, worktree, since, env),
  }
}

/// Poll for the session for up to `wait`: start readiness can precede the
/// host's own session persistence.
///
/// # Errors
/// No single session appeared, or a store cannot be read.
pub(crate) fn await_session(
  kind: HostKind,
  worktree: &str,
  since: u64,
  env: &Env,
  wait: Duration,
) -> Result<String, Failure> {
  let deadline = Instant::now() + wait;
  loop {
    if let Some(id) = capture_session(kind, worktree, since, env)? {
      return Ok(id);
    }
    if Instant::now() >= deadline {
      return Err(Failure::unknown(format!(
        "cannot attest {} session identity; inspect the owned pane before retry",
        kind.name()
      )));
    }
    std::thread::sleep(Duration::from_millis(500));
  }
}

/// Refuse to reuse a session unless the host's own records name exactly it.
///
/// # Errors
/// The captured session is another, or none.
pub(crate) fn require_captured_session(
  kind: HostKind,
  worktree: &str,
  since: u64,
  expected: &str,
  env: &Env,
) -> Result<(), String> {
  if capture_session(kind, worktree, since, env)?.as_deref() == Some(expected) {
    Ok(())
  } else {
    Err("started agent did not attach the recorded session".to_owned())
  }
}

#[cfg(test)]
#[path = "tests/session_test.rs"]
mod tests;
