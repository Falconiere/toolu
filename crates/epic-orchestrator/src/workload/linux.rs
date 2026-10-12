//! The Linux inventory from `/proc`: identities of every process, cwds only
//! of this user's, so a root-free run still sees foreign-UID descendants.

use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;

use super::ProcessRecord;

/// One `/proc/<pid>/stat` line as a record; `None` for a zombie.
///
/// # Errors
/// The line is not a stat line.
pub(super) fn parse_stat(pid: u32, stat: &str) -> Result<Option<ProcessRecord>, String> {
  let invalid = || format!("invalid /proc stat for process {pid}");
  let rest = stat
    .rfind(')')
    .and_then(|close| stat.get(close + 1..))
    .ok_or_else(invalid)?;
  let fields: Vec<&str> = rest.split_whitespace().collect();
  let state = fields.first().copied().ok_or_else(invalid)?;
  let parent = fields
    .get(1)
    .and_then(|field| field.parse::<u32>().ok())
    .ok_or_else(invalid)?;
  let group = fields
    .get(2)
    .and_then(|field| field.parse::<u32>().ok())
    .ok_or_else(invalid)?;
  let started = fields.get(19).copied().ok_or_else(invalid)?;
  if state == "Z" {
    return Ok(None);
  }
  Ok(Some(ProcessRecord {
    pid,
    parent,
    group,
    started: format!("linux:{started}"),
    cwd: None,
    cwd_unavailable: false,
  }))
}

/// The live record of `pid`; `None` once it is gone or a zombie.
///
/// # Errors
/// The stat file cannot be read for another reason, or is malformed.
pub(super) fn record(pid: u32) -> Result<Option<ProcessRecord>, String> {
  match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
    Ok(stat) => parse_stat(pid, &stat),
    Err(err) if err.kind() == ErrorKind::NotFound || err.raw_os_error() == Some(3) => Ok(None),
    Err(err) => Err(format!("cannot inspect process {pid}: {err}")),
  }
}

/// Every live process, with the cwd of this user's.
///
/// # Errors
/// `/proc` cannot be listed, or a process cannot be inspected.
pub(super) fn processes() -> Result<Vec<ProcessRecord>, String> {
  let uid = std::fs::metadata("/proc/self")
    .map_err(|err| format!("Linux process uid is unavailable: {err}"))?
    .uid();
  let entries = std::fs::read_dir("/proc").map_err(|err| format!("cannot list /proc: {err}"))?;
  let mut records = Vec::new();
  for entry in entries.filter_map(Result::ok) {
    let Some(pid) = entry
      .file_name()
      .to_str()
      .and_then(|name| name.parse::<u32>().ok())
    else {
      continue;
    };
    let owner = match entry.metadata() {
      Ok(meta) => meta.uid(),
      Err(err) if err.kind() == ErrorKind::NotFound => continue,
      Err(err) => return Err(format!("cannot inspect process owner {pid}: {err}")),
    };
    let Some(mut found) = record(pid)? else {
      continue;
    };
    if owner == uid {
      read_cwd(&mut found)?;
    } else {
      found.cwd_unavailable = true;
    }
    records.push(found);
  }
  Ok(records)
}

fn read_cwd(found: &mut ProcessRecord) -> Result<(), String> {
  match std::fs::read_link(format!("/proc/{}/cwd", found.pid)) {
    Ok(cwd) => found.cwd = Some(cwd.display().to_string()),
    Err(err) if err.kind() == ErrorKind::PermissionDenied => found.cwd_unavailable = true,
    Err(err) if err.kind() == ErrorKind::NotFound => {}
    Err(err) => return Err(format!("cannot inspect process {} cwd: {err}", found.pid)),
  }
  Ok(())
}

#[cfg(test)]
#[path = "tests/linux_test.rs"]
mod tests;
