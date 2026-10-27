//! Claude Code task output discovery and bounded reads.

use std::fs::{self, File};
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use toolu_runtime::config::secrets::{Secrets, redact_window, redaction_overlap};
use toolu_state::time::iso_millis;

use super::{Chunk, Log, Source};
use crate::model::Issue;

const TASK_FRESH: Duration = Duration::from_secs(3);

fn safe_component(text: &str) -> bool {
  !text.is_empty()
    && text
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn regular_dir(path: &Path) -> bool {
  fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_dir())
}

/// The platform's temporary root used by Claude Code.
pub(super) fn claude_temp() -> &'static str {
  if cfg!(target_os = "macos") {
    "/private/tmp"
  } else {
    "/tmp"
  }
}

/// The task root for this worker's exact worktree slug.
pub(super) fn claude_root(issue: &Issue) -> Option<PathBuf> {
  if issue.kind != "claude" {
    return None;
  }
  let worktree = issue.worktree.as_deref()?;
  if !regular_dir(Path::new(worktree)) {
    return None;
  }
  let slug: String = worktree
    .chars()
    .map(|ch| if matches!(ch, '/' | '.') { '-' } else { ch })
    .collect();
  Some(
    PathBuf::from(claude_temp())
      .join(format!("claude-{}", toolu_runtime::machine::user_id()))
      .join(slug),
  )
}

fn entries(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
  match fs::read_dir(path) {
    Ok(entries) => entries
      .map(|entry| entry.map_err(|err| err.to_string()))
      .collect(),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
    Err(err) => Err(format!("{}: {err}", path.display())),
  }
}

fn open_regular(path: &Path) -> Result<File, String> {
  let before = fs::symlink_metadata(path).map_err(|err| err.to_string())?;
  if !before.file_type().is_file() {
    return Err("invalid task file".to_owned());
  }
  let file = File::open(path).map_err(|err| err.to_string())?;
  let opened = file.metadata().map_err(|err| err.to_string())?;
  if before.dev() != opened.dev() || before.ino() != opened.ino() {
    return Err("task file changed".to_owned());
  }
  Ok(file)
}

fn first_line(path: &Path, fallback: &str, secrets: &Secrets) -> String {
  let mut bytes = Vec::new();
  let limit = 4096_usize.saturating_add(redaction_overlap(secrets));
  let read = open_regular(path).and_then(|file| {
    file
      .take(limit as u64)
      .read_to_end(&mut bytes)
      .map_err(|err| err.to_string())
  });
  if read.is_err() {
    return fallback.to_owned();
  }
  let end = bytes
    .iter()
    .position(|byte| *byte == b'\n')
    .unwrap_or(bytes.len());
  if end >= 4096 {
    return fallback.to_owned();
  }
  let shown = redact_window(&bytes, 0, end, secrets);
  let line = shown.trim();
  if line.is_empty() {
    fallback.to_owned()
  } else {
    line.to_owned()
  }
}

/// List regular task output files beneath the worker's task root.
pub(super) fn claude_logs(issue: &Issue, secrets: &Secrets) -> Result<Vec<Log>, String> {
  let Some(root) = claude_root(issue).filter(|path| regular_dir(path)) else {
    return Ok(Vec::new());
  };
  let mut logs = Vec::new();
  for session in entries(&root)? {
    let name = session.file_name().to_string_lossy().into_owned();
    if !safe_component(&name) || !session.file_type().map_err(|err| err.to_string())?.is_dir() {
      continue;
    }
    let tasks = session.path().join("tasks");
    if !regular_dir(&tasks) {
      continue;
    }
    for task in entries(&tasks)? {
      let filename = task.file_name().to_string_lossy().into_owned();
      let Some(id) = filename
        .strip_suffix(".output")
        .filter(|id| safe_component(id))
      else {
        continue;
      };
      if !task.file_type().map_err(|err| err.to_string())?.is_file() {
        continue;
      }
      let path = task.path();
      let meta = fs::symlink_metadata(&path).map_err(|err| err.to_string())?;
      let updated = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
      let started = meta.created().unwrap_or(updated);
      logs.push(Log {
        id: format!("claude-{name}-{id}"),
        source: Source::Claude(path.clone()),
        label: first_line(&path, id, secrets),
        size: meta.len(),
        state: if issue.stage == "running" && updated.elapsed().unwrap_or_default() <= TASK_FRESH {
          "running"
        } else {
          "done"
        },
        started_at: iso_millis(started),
        updated_at: iso_millis(updated),
      });
    }
  }
  Ok(logs)
}

/// Read a bounded raw window from a selected regular task file.
pub(super) fn read_claude(path: &Path, requested: u64, limit: usize) -> Result<Chunk, String> {
  let mut file = open_regular(path)?;
  let meta = file.metadata().map_err(|err| err.to_string())?;
  let size = meta.len();
  let offset = requested.min(size);
  file
    .seek(SeekFrom::Start(offset))
    .map_err(|err| err.to_string())?;
  let mut bytes = Vec::new();
  file
    .take(limit as u64)
    .read_to_end(&mut bytes)
    .map_err(|err| err.to_string())?;
  let next_offset = offset.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
  Ok(Chunk {
    bytes,
    offset,
    next_offset,
    size,
    truncated: false,
  })
}

#[cfg(test)]
#[path = "tests/claude_test.rs"]
mod tests;
