//! User-only epic credentials; no value may enter a diagnostic or public view.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use super::epic::credential_key;
use crate::host::roots::Roots;

const REDACTED: &str = "<redacted>";

/// A failure that never includes credential bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError(&'static str);

impl fmt::Display for SecretError {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    output.write_str(self.0)
  }
}

/// Resolved credentials, with environment values winning per field.
pub struct Secrets {
  status_token: Option<String>,
  notify_url: Option<String>,
  peer_tokens: BTreeMap<String, String>,
}

impl fmt::Debug for Secrets {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    output.write_str(
      "Secrets { status_token: <redacted>, notify_url: <redacted>, peer_tokens: <redacted> }",
    )
  }
}

impl Secrets {
  /// The token required for a non-loopback status listener, if configured.
  pub fn status_token(&self) -> Option<&str> {
    self.status_token.as_deref()
  }

  /// The outgoing attention endpoint, if configured.
  pub fn notify_url(&self) -> Option<&str> {
    self.notify_url.as_deref()
  }

  /// A named peer's bearer token, if configured.
  pub fn peer_token(&self, name: &str) -> Option<&str> {
    self.peer_tokens.get(name).map(String::as_str)
  }

  fn values(&self) -> Vec<&str> {
    let mut values: Vec<&str> = self
      .status_token()
      .into_iter()
      .chain(self.notify_url())
      .chain(self.peer_tokens.values().map(String::as_str))
      .collect();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values
  }
}

/// The one user-only secret file for a host's config root.
pub fn path(roots: &Roots) -> PathBuf {
  roots.config_root().join("toolu/secrets.json")
}

fn file_error() -> SecretError {
  SecretError("epic secrets: cannot read user-only secrets.json")
}

fn invalid_file() -> SecretError {
  SecretError("epic secrets: invalid secrets.json structure")
}

fn read_file(path: &Path) -> Result<Option<Map<String, Value>>, SecretError> {
  match fs::symlink_metadata(path) {
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(_) => return Err(file_error()),
    Ok(meta) if !meta.file_type().is_file() => return Err(file_error()),
    Ok(_) => {}
  }
  let mut file = OpenOptions::new()
    .read(true)
    .custom_flags(nix::libc::O_NOFOLLOW)
    .open(path)
    .map_err(|_| file_error())?;
  let mode = file
    .metadata()
    .map_err(|_| file_error())?
    .permissions()
    .mode()
    & 0o777;
  if mode & 0o077 != 0 || mode & 0o400 == 0 {
    return Err(SecretError(
      "epic secrets: unsafe permissions; run chmod 600 on secrets.json",
    ));
  }
  let mut text = String::new();
  file.read_to_string(&mut text).map_err(|_| file_error())?;
  let value: Value = serde_json::from_str(&text).map_err(|_| invalid_file())?;
  let map = value.as_object().ok_or_else(invalid_file)?.clone();
  if map.get("version").and_then(Value::as_u64) != Some(1) {
    return Err(invalid_file());
  }
  optional_string(&map, "status_token")?;
  optional_string(&map, "notify_url")?;
  peer_tokens(&map)?;
  Ok(Some(map))
}

fn optional_string(map: &Map<String, Value>, key: &str) -> Result<Option<String>, SecretError> {
  map
    .get(key)
    .map(|value| {
      value
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(invalid_file)
    })
    .transpose()
}

fn peer_tokens(map: &Map<String, Value>) -> Result<BTreeMap<String, String>, SecretError> {
  let Some(value) = map.get("peer_tokens") else {
    return Ok(BTreeMap::new());
  };
  let object = value.as_object().ok_or_else(invalid_file)?;
  object
    .iter()
    .map(|(name, value)| {
      let Some(token) = value.as_str().filter(|token| !token.is_empty()) else {
        return Err(invalid_file());
      };
      if name.is_empty() {
        return Err(invalid_file());
      }
      Ok((name.clone(), token.to_owned()))
    })
    .collect()
}

fn env_peer_tokens(roots: &Roots) -> Result<BTreeMap<String, String>, SecretError> {
  let Some(raw) = roots
    .env()
    .get("TOOLU_EPIC_PEER_TOKENS")
    .filter(|value| !value.is_empty())
  else {
    return Ok(BTreeMap::new());
  };
  let value: Value = serde_json::from_str(raw)
    .map_err(|_| SecretError("epic secrets: invalid TOOLU_EPIC_PEER_TOKENS object"))?;
  let map = value.as_object().ok_or(SecretError(
    "epic secrets: invalid TOOLU_EPIC_PEER_TOKENS object",
  ))?;
  peer_tokens(&Map::from_iter([(
    "peer_tokens".to_owned(),
    Value::Object(map.clone()),
  )]))
  .map_err(|_| SecretError("epic secrets: invalid TOOLU_EPIC_PEER_TOKENS object"))
}

/// Read file credentials, then apply environment overrides per field.
///
/// # Errors
/// Refuses unsafe mode, symlinks, malformed data and invalid environment JSON.
pub fn load(roots: &Roots) -> Result<Secrets, SecretError> {
  let map = read_file(&path(roots))?.unwrap_or_default();
  let env = roots.env();
  let status_token = env
    .get("TOOLU_EPIC_STATUS_TOKEN")
    .filter(|value| !value.is_empty())
    .or_else(|| {
      env
        .get("TOOLU_EPIC_TOKEN")
        .filter(|value| !value.is_empty())
    })
    .map_or_else(
      || optional_string(&map, "status_token"),
      |value| Ok(Some(value.to_owned())),
    )?;
  let notify_url = env
    .get("TOOLU_EPIC_NOTIFY_URL")
    .filter(|value| !value.is_empty())
    .map_or_else(
      || optional_string(&map, "notify_url"),
      |value| Ok(Some(value.to_owned())),
    )?;
  let mut tokens = peer_tokens(&map)?;
  tokens.extend(env_peer_tokens(roots)?);
  Ok(Secrets {
    status_token,
    notify_url,
    peer_tokens: tokens,
  })
}

fn random_token() -> Result<String, SecretError> {
  let mut random = File::open("/dev/urandom")
    .map_err(|_| SecretError("epic secrets: OS random source unavailable"))?;
  let mut bytes = [0_u8; 32];
  random
    .read_exact(&mut bytes)
    .map_err(|_| SecretError("epic secrets: OS random source unavailable"))?;
  let mut token = String::with_capacity(64);
  for byte in bytes {
    use std::fmt::Write as _;
    write!(token, "{byte:02x}").map_err(|_| SecretError("epic secrets: token encoding failed"))?;
  }
  Ok(token)
}

fn temp_path(path: &Path) -> PathBuf {
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_nanos());
  let mut name = path.as_os_str().to_owned();
  name.push(format!(".{}.{nanos}.tmp", std::process::id()));
  PathBuf::from(name)
}

fn write_file(path: &Path, map: &Map<String, Value>) -> Result<(), SecretError> {
  let parent = path.parent().ok_or_else(file_error)?;
  fs::create_dir_all(parent).map_err(|_| file_error())?;
  let temp = temp_path(path);
  let result = (|| {
    let mut file = OpenOptions::new()
      .write(true)
      .create_new(true)
      .mode(0o600)
      .open(&temp)
      .map_err(|_| file_error())?;
    let text = Value::Object(map.clone()).to_string();
    file.write_all(text.as_bytes()).map_err(|_| file_error())?;
    file.write_all(b"\n").map_err(|_| file_error())?;
    file.sync_all().map_err(|_| file_error())?;
    fs::rename(&temp, path).map_err(|_| file_error())
  })();
  if result.is_err() {
    let _ = fs::remove_file(temp);
  }
  result
}

/// Atomically replace the status token, keeping other file fields unchanged.
///
/// # Errors
/// Refuses an unsafe existing file, random-source failure or failed write.
pub fn rotate_status_token(roots: &Roots) -> Result<PathBuf, SecretError> {
  for variable in ["TOOLU_EPIC_STATUS_TOKEN", "TOOLU_EPIC_TOKEN"] {
    if roots
      .env()
      .get(variable)
      .is_some_and(|value| !value.is_empty())
    {
      return Err(SecretError(match variable {
        "TOOLU_EPIC_STATUS_TOKEN" => {
          "epic secrets: unset TOOLU_EPIC_STATUS_TOKEN before token rotation"
        }
        _ => "epic secrets: unset TOOLU_EPIC_TOKEN before token rotation",
      }));
    }
  }
  let path = path(roots);
  let mut map = read_file(&path)?.unwrap_or_default();
  map.insert("version".to_owned(), Value::from(1));
  map.insert("status_token".to_owned(), Value::String(random_token()?));
  write_file(&path, &map)?;
  Ok(path)
}

/// Replace every resolved secret substring in ordinary text.
pub fn redact_text(text: &str, secrets: &Secrets) -> String {
  secrets
    .values()
    .into_iter()
    .fold(text.to_owned(), |shown, value| {
      shown.replace(value, REDACTED)
    })
}

/// Replace credential-named JSON fields and resolved secret substrings.
pub fn redact_json(value: &Value, secrets: &Secrets) -> Value {
  match value {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .map(|(key, value)| {
          let shown = if credential_key(key) {
            Value::String(REDACTED.to_owned())
          } else {
            redact_json(value, secrets)
          };
          (key.clone(), shown)
        })
        .collect(),
    ),
    Value::Array(items) => Value::Array(
      items
        .iter()
        .map(|item| redact_json(item, secrets))
        .collect(),
    ),
    Value::String(text) => Value::String(redact_text(text, secrets)),
    _ => value.clone(),
  }
}

#[cfg(test)]
#[path = "tests/secrets_test.rs"]
mod tests;
