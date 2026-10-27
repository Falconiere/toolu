//! Bounded logs for managed jobs under `<resource_root>/jobs/logs`.

use std::fs::{self, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_state::lock::token;
use toolu_state::time::iso_millis;

mod catalog;

/// Maximum bytes retained in one job's physical log file.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
const RETAIN: Duration = Duration::from_hours(24);
const KEEP_PER_KEY: usize = 20;

/// A discovered log, including its logical byte window and owner.
#[derive(Clone)]
pub struct Entry {
  /// Opaque API identifier.
  pub id: String,
  /// Owning worker key.
  pub key: String,
  /// Original command line.
  pub label: String,
  /// Lease-derived running or done state.
  pub state: &'static str,
  /// Bytes ever written, including the discarded prefix.
  pub size: u64,
  /// Job start time.
  pub started_at: String,
  /// Last file modification time.
  pub updated_at: String,
  path: PathBuf,
  updated: SystemTime,
}

/// A bounded read from a log's logical byte stream.
pub struct Chunk {
  /// Bytes in this page.
  pub bytes: Vec<u8>,
  /// Effective first byte offset.
  pub offset: u64,
  /// Cursor after the bytes returned.
  pub next_offset: u64,
  /// Total logical bytes written.
  pub size: u64,
  /// The requested offset was before the kept window.
  pub truncated: bool,
}

/// A writer identified by the job's lease token.
pub struct Writer {
  path: PathBuf,
  meta: PathBuf,
  lock: PathBuf,
}

fn dir(root: &Path) -> PathBuf {
  root.join("jobs/logs")
}

fn io(path: &Path, err: impl std::fmt::Display) -> String {
  format!("{}: {err}", path.display())
}

fn meta_path(path: &Path) -> PathBuf {
  path.with_extension("json")
}

fn lock_path(path: &Path) -> PathBuf {
  path.with_extension("lock")
}

fn lock(path: &Path) -> Result<std::fs::File, String> {
  let file = OpenOptions::new()
    .create(true)
    .append(true)
    .mode(0o600)
    .open(path)
    .map_err(|err| io(path, err))?;
  file.lock().map_err(|err| io(path, err))?;
  Ok(file)
}

fn metadata(path: &Path) -> Result<Value, String> {
  let bytes = fs::read(path).map_err(|err| io(path, err))?;
  serde_json::from_slice(&bytes).map_err(|err| io(path, err))
}

fn save_meta(path: &Path, value: &Value) -> Result<(), String> {
  let temp = path.with_extension(format!("{}.tmp", token()));
  let bytes = serde_json::to_vec(value).map_err(|err| io(path, err))?;
  let result = (|| {
    let mut file = OpenOptions::new()
      .write(true)
      .create_new(true)
      .mode(0o600)
      .open(&temp)
      .map_err(|err| io(&temp, err))?;
    file.write_all(&bytes).map_err(|err| io(&temp, err))?;
    fs::rename(&temp, path).map_err(|err| io(path, err))
  })();
  if result.is_err() {
    let _removed = fs::remove_file(&temp);
  }
  result
}

/// Create a log and its metadata after the job lease is acquired.
///
/// # Errors
/// The log directory, file, metadata or retention cannot be written.
pub fn start(root: &Path, lease_token: &str, key: &str, argv: &[String]) -> Result<Writer, String> {
  let folder = dir(root);
  fs::create_dir_all(&folder).map_err(|err| io(&folder, err))?;
  let path = folder.join(format!("{lease_token}.log"));
  let file = OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(&path)
    .map_err(|err| io(&path, err))?;
  drop(file);
  let meta = meta_path(&path);
  let value = json!({
    "key": key, "label": argv.join(" "),
    "started_at": iso_millis(SystemTime::now()), "base_offset": 0,
  });
  if let Err(err) = save_meta(&meta, &value) {
    let _removed = fs::remove_file(&path);
    return Err(err);
  }
  let writer = Writer {
    lock: lock_path(&path),
    path,
    meta,
  };
  if let Err(err) = list(root, key) {
    let _removed_log = fs::remove_file(&writer.path);
    let _removed_meta = fs::remove_file(&writer.meta);
    let _removed_lock = fs::remove_file(&writer.lock);
    return Err(err);
  }
  Ok(writer)
}

impl Writer {
  /// Append one raw chunk, keeping only the newest 8 MiB on disk.
  ///
  /// # Errors
  /// The log, lock or metadata cannot be read or written.
  pub fn append(&self, bytes: &[u8]) -> Result<(), String> {
    let _held = lock(&self.lock)?;
    let old = fs::metadata(&self.path)
      .map_err(|err| io(&self.path, err))?
      .len();
    let added = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if old.saturating_add(added) <= MAX_BYTES as u64 {
      return OpenOptions::new()
        .append(true)
        .open(&self.path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|err| io(&self.path, err));
    }
    self.write_ring(bytes, old, added)
  }

  fn write_ring(&self, bytes: &[u8], old: u64, added: u64) -> Result<(), String> {
    let mut meta = metadata(&self.meta)?;
    let base = meta
      .get("base_offset")
      .and_then(Value::as_u64)
      .ok_or("invalid job log metadata")?;
    let total = base.saturating_add(old).saturating_add(added);
    let kept = bytes
      .get(bytes.len().saturating_sub(MAX_BYTES)..)
      .unwrap_or(bytes);
    let start = total.saturating_sub(u64::try_from(kept.len()).unwrap_or(u64::MAX));
    let mut file = OpenOptions::new()
      .write(true)
      .open(&self.path)
      .map_err(|err| io(&self.path, err))?;
    file
      .set_len(MAX_BYTES as u64)
      .map_err(|err| io(&self.path, err))?;
    let position = usize::try_from(start % MAX_BYTES as u64).unwrap_or_default();
    let first = kept.len().min(MAX_BYTES - position);
    let (front, back) = kept.split_at(first);
    file
      .seek(SeekFrom::Start(position as u64))
      .and_then(|_| file.write_all(front))
      .and_then(|()| file.seek(SeekFrom::Start(0)))
      .and_then(|_| file.write_all(back))
      .map_err(|err| io(&self.path, err))?;
    // shortcut: a crash between ring writes and metadata rename can lose newest bytes; use transactional generations if crash-consistent tails are required.
    meta
      .as_object_mut()
      .ok_or("invalid job log metadata")?
      .insert("base_offset".to_owned(), json!(total - MAX_BYTES as u64));
    save_meta(&self.meta, &meta)
  }
}

pub use catalog::list;

/// Read one discovered log under its writer's lock.
///
/// # Errors
/// The selected file or metadata cannot be read safely.
pub fn read(entry: &Entry, requested: Option<u64>, limit: usize) -> Result<Chunk, String> {
  let _held = lock(&lock_path(&entry.path))?;
  let info = fs::symlink_metadata(&entry.path).map_err(|err| io(&entry.path, err))?;
  if !info.file_type().is_file() || info.len() > MAX_BYTES as u64 {
    return Err("invalid job log file".to_owned());
  }
  let meta = metadata(&meta_path(&entry.path))?;
  let base = meta
    .get("base_offset")
    .and_then(Value::as_u64)
    .ok_or("invalid job log metadata")?;
  let size = base.saturating_add(info.len());
  let wanted = requested.unwrap_or_else(|| size.saturating_sub(64 * 1024));
  let truncated = requested.is_some_and(|offset| offset < base);
  let offset = wanted.clamp(base, size);
  let mut file = fs::File::open(&entry.path).map_err(|err| io(&entry.path, err))?;
  let opened = file.metadata().map_err(|err| io(&entry.path, err))?;
  if info.dev() != opened.dev() || info.ino() != opened.ino() {
    return Err("job log changed during read".to_owned());
  }
  let position = if info.len() == MAX_BYTES as u64 {
    offset % MAX_BYTES as u64
  } else {
    offset - base
  };
  let count = usize::try_from((size - offset).min(limit as u64)).unwrap_or_default();
  let first = count.min(usize::try_from(info.len() - position).unwrap_or_default());
  let mut bytes = read_part(&mut file, &entry.path, position, first)?;
  bytes.extend(read_part(&mut file, &entry.path, 0, count - first)?);
  Ok(Chunk {
    bytes,
    offset,
    next_offset: offset.saturating_add(count as u64),
    size,
    truncated,
  })
}

fn read_part(
  file: &mut fs::File,
  path: &Path,
  position: u64,
  count: usize,
) -> Result<Vec<u8>, String> {
  let mut bytes = vec![0; count];
  file
    .seek(SeekFrom::Start(position))
    .and_then(|_| file.read_exact(&mut bytes))
    .map_err(|err| io(path, err))?;
  Ok(bytes)
}

#[cfg(test)]
#[path = "tests/job_logs_test.rs"]
mod tests;
