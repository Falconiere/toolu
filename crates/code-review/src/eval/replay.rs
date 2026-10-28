//! The replay store: a directory of `<stage>-<sha256>.json` files, each holding the
//! response body of one recorded exchange. The key is the stage and the SHA-256 of
//! the exact masked request text the recorder emits, so a replay can only answer a
//! request that is byte-for-byte the one recorded. The default eval mode reads it;
//! `--live` with a replay directory writes it.

use std::path::{Path, PathBuf};

use ring::digest::{SHA256, digest};
use serde_json::Value;

use crate::review::Stage;

/// A directory of recorded responses.
#[derive(Debug, Clone)]
pub struct Store {
  dir: PathBuf,
}

/// Lowercase hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
  let digit = |nibble: u8| char::from_digit(u32::from(nibble), 16);
  bytes
    .iter()
    .flat_map(|byte| [digit(byte >> 4), digit(byte & 0x0f)])
    .flatten()
    .collect()
}

impl Store {
  /// The store in `dir`; the directory is created on the first write.
  pub fn new(dir: &Path) -> Store {
    Store {
      dir: dir.to_path_buf(),
    }
  }

  /// The file stem of an exchange: `<stage>-<sha256 of request>`.
  pub fn key(stage: Stage, request: &str) -> String {
    format!(
      "{}-{}",
      stage.name(),
      hex(digest(&SHA256, request.as_bytes()).as_ref())
    )
  }

  fn file(&self, stage: Stage, request: &str) -> PathBuf {
    self
      .dir
      .join(format!("{}.json", Store::key(stage, request)))
  }

  /// The recorded response to `request`, or `None` when none was recorded.
  ///
  /// # Errors
  /// A recorded file that exists but cannot be read.
  pub fn read(&self, stage: Stage, request: &str) -> Result<Option<String>, String> {
    let file = self.file(stage, request);
    match std::fs::read_to_string(&file) {
      Ok(body) => Ok(Some(body)),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(format!("{}: {error}", file.display())),
    }
  }

  /// Record `response` as the answer to `request`, replacing an earlier one,
  /// without the provider's top-level `id`.
  ///
  /// # Errors
  /// The directory or file cannot be written.
  pub fn write(&self, stage: Stage, request: &str, response: &str) -> Result<(), String> {
    std::fs::create_dir_all(&self.dir)
      .map_err(|error| format!("{}: {error}", self.dir.display()))?;
    let file = self.file(stage, request);
    std::fs::write(&file, without_id(response))
      .map_err(|error| format!("{}: {error}", file.display()))
  }
}

/// `response` without a JSON object's top-level `id`: the provider's random
/// per-request token, which no client reads and which the secrets scanner flags
/// once recordings are committed. Any other text is kept as it is.
fn without_id(response: &str) -> String {
  let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(response) else {
    return response.to_owned();
  };
  if fields.remove("id").is_none() {
    return response.to_owned();
  }
  Value::Object(fields).to_string()
}

#[cfg(test)]
#[path = "tests/replay_test.rs"]
mod tests;
