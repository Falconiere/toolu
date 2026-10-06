//! Non-secret epic engine settings in the open `epic` config namespace.

use std::collections::HashSet;
use std::fmt;

use serde_json::{Map, Value};
use url::Url;

use super::load::LoadedConfig;

/// One credential-free peer origin; its token is resolved separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
  /// The name used to find this peer's token.
  pub name: String,
  /// An HTTP(S) origin with no user info, path, query or fragment.
  pub url: String,
}

/// Non-secret engine settings from `toolu.config.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicSettings {
  /// Status listener bind address.
  pub http_bind: String,
  /// Status listener port.
  pub http_port: u16,
  /// Whether outgoing attention notifications are enabled.
  pub attention_enabled: bool,
  /// Read-only peer engines.
  pub peers: Vec<Peer>,
}

/// A bad setting; values are deliberately absent from diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicError {
  path: String,
  reason: &'static str,
}

impl fmt::Display for EpicError {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(output, "{}: {}", self.path, self.reason)
  }
}

fn bad(path: impl Into<String>, reason: &'static str) -> EpicError {
  EpicError {
    path: path.into(),
    reason,
  }
}

fn section<'a>(
  root: &'a Map<String, Value>,
  key: &str,
  path: &str,
) -> Result<Option<&'a Map<String, Value>>, EpicError> {
  root
    .get(key)
    .map(|value| {
      value
        .as_object()
        .ok_or_else(|| bad(path, "expected an object"))
    })
    .transpose()
}

fn credential_key(key: &str) -> bool {
  let normalized: String = key
    .chars()
    .filter(char::is_ascii_alphanumeric)
    .flat_map(char::to_lowercase)
    .collect();
  ["token", "secret", "password", "notifyurl"]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn reject_credentials(value: &Value, path: &str) -> Result<(), EpicError> {
  match value {
    Value::Object(map) => {
      for (key, value) in map {
        let at = format!("{path}.{key}");
        if credential_key(key) {
          return Err(bad(at, "credential belongs in toolu/secrets.json"));
        }
        reject_credentials(value, &at)?;
      }
    }
    Value::Array(items) => {
      for (index, item) in items.iter().enumerate() {
        reject_credentials(item, &format!("{path}[{index}]"))?;
      }
    }
    _ => {}
  }
  Ok(())
}

fn parse_port(http: Option<&Map<String, Value>>) -> Result<u16, EpicError> {
  let Some(value) = http.and_then(|http| http.get("port")) else {
    return Ok(7717);
  };
  value
    .as_u64()
    .and_then(|port| u16::try_from(port).ok())
    .filter(|port| *port > 0)
    .ok_or_else(|| bad("epic.http.port", "expected an integer from 1 to 65535"))
}

fn parse_bind(http: Option<&Map<String, Value>>) -> Result<String, EpicError> {
  let Some(value) = http.and_then(|http| http.get("bind")) else {
    return Ok("127.0.0.1".to_owned());
  };
  value
    .as_str()
    .filter(|bind| !bind.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| bad("epic.http.bind", "expected a nonempty string"))
}

fn parse_attention(attention: Option<&Map<String, Value>>) -> Result<bool, EpicError> {
  let Some(value) = attention.and_then(|attention| attention.get("enabled")) else {
    return Ok(false);
  };
  value
    .as_bool()
    .ok_or_else(|| bad("epic.attention.enabled", "expected a boolean"))
}

fn peer_url(value: &Value, path: &str) -> Result<String, EpicError> {
  let raw = value
    .as_str()
    .ok_or_else(|| bad(path, "expected a credential-free HTTP(S) origin"))?;
  let url = Url::parse(raw).map_err(|_| bad(path, "expected a credential-free HTTP(S) origin"))?;
  let safe = matches!(url.scheme(), "http" | "https")
    && url.host_str().is_some()
    && url.username().is_empty()
    && url.password().is_none()
    && url.path() == "/"
    && url.query().is_none()
    && url.fragment().is_none();
  if !safe {
    return Err(bad(path, "expected a credential-free HTTP(S) origin"));
  }
  Ok(url.to_string())
}

fn parse_peers(epic: &Map<String, Value>) -> Result<Vec<Peer>, EpicError> {
  let Some(value) = epic.get("peers") else {
    return Ok(Vec::new());
  };
  let items = value
    .as_array()
    .ok_or_else(|| bad("epic.peers", "expected an array"))?;
  let mut names = HashSet::new();
  let mut peers = Vec::new();
  for (index, item) in items.iter().enumerate() {
    let path = format!("epic.peers[{index}]");
    let object = item
      .as_object()
      .ok_or_else(|| bad(&path, "expected an object"))?;
    let name = object
      .get("name")
      .and_then(Value::as_str)
      .filter(|name| !name.trim().is_empty())
      .ok_or_else(|| bad(format!("{path}.name"), "expected a nonempty string"))?;
    if !names.insert(name) {
      return Err(bad(format!("{path}.name"), "duplicate peer name"));
    }
    let url = object
      .get("url")
      .ok_or_else(|| bad(format!("{path}.url"), "missing URL"))?;
    peers.push(Peer {
      name: name.to_owned(),
      url: peer_url(url, &format!("{path}.url"))?,
    });
  }
  Ok(peers)
}

/// Interpret this binary's known engine settings, ignoring unknown safe keys.
///
/// # Errors
/// Rejects an invalid config envelope, credential-looking key or bad known value.
pub fn parse(config: &LoadedConfig) -> Result<EpicSettings, EpicError> {
  if config.invalid.is_some() {
    return Err(bad("epic", "config envelope invalid"));
  }
  let Some(value) = config.data.get("epic") else {
    return Ok(EpicSettings {
      http_bind: "127.0.0.1".to_owned(),
      http_port: 7717,
      attention_enabled: false,
      peers: Vec::new(),
    });
  };
  reject_credentials(value, "epic")?;
  let epic = value
    .as_object()
    .ok_or_else(|| bad("epic", "expected an object"))?;
  let http = section(epic, "http", "epic.http")?;
  let attention = section(epic, "attention", "epic.attention")?;
  Ok(EpicSettings {
    http_bind: parse_bind(http)?,
    http_port: parse_port(http)?,
    attention_enabled: parse_attention(attention)?,
    peers: parse_peers(epic)?,
  })
}

#[cfg(test)]
#[path = "tests/epic_test.rs"]
mod tests;
