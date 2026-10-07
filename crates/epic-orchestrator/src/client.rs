//! One request on the engine socket.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use crate::disk::write_value;
use crate::paths::Paths;

/// Send `request` and return the engine's one-line reply.
///
/// # Errors
/// The socket cannot be reached or the reply is not JSON.
pub(crate) fn exchange(paths: &Paths, protocol: u64, request: &Value) -> Result<Value, String> {
  let mut stream = UnixStream::connect(paths.socket()).map_err(|err| err.to_string())?;
  let hello = read_json(&mut stream)?;
  let server = hello.get("protocol").and_then(Value::as_u64).unwrap_or(0);
  if server != protocol {
    spool(paths, request)?;
    write_json(&mut stream, &json!({"op": "replace", "protocol": server}))?;
    return read_json(&mut stream);
  }
  if request.get("op").and_then(Value::as_str) == Some("report") {
    spool(paths, request)?;
  }
  write_json(&mut stream, &with_protocol(request, protocol))?;
  read_json(&mut stream)
}

/// Retry `exchange` while the engine is binding its socket.
///
/// # Errors
/// Every attempt failed. The last error is returned.
pub(crate) fn exchange_retry(
  paths: &Paths,
  protocol: u64,
  request: &Value,
) -> Result<Value, String> {
  let mut last = "engine socket is not open".to_owned();
  for _ in 0..50 {
    match exchange(paths, protocol, request) {
      Ok(value) => return Ok(value),
      Err(err) => last = err,
    }
    thread::sleep(Duration::from_millis(20));
  }
  Err(last)
}

pub(crate) fn read_json(stream: &mut UnixStream) -> Result<Value, String> {
  let line = read_line(stream)?;
  serde_json::from_str(&line).map_err(|err| err.to_string())
}

pub(crate) fn write_json(stream: &mut UnixStream, value: &Value) -> Result<(), String> {
  let mut line = serde_json::to_string(value).map_err(|err| err.to_string())?;
  line.push('\n');
  stream
    .write_all(line.as_bytes())
    .map_err(|err| err.to_string())
}

fn with_protocol(request: &Value, protocol: u64) -> Value {
  let mut body = request.clone();
  if let Some(map) = body.as_object_mut() {
    map.insert("protocol".to_owned(), json!(protocol));
  }
  body
}

fn spool(paths: &Paths, request: &Value) -> Result<(), String> {
  let token = request
    .get("token")
    .and_then(Value::as_str)
    .unwrap_or("report");
  write_value(&spool_path(&paths.root, token)?, request)
}

fn read_line(stream: &mut UnixStream) -> Result<String, String> {
  let mut buf = Vec::new();
  let mut byte = [0_u8; 1];
  while buf.len() < 65_536 {
    let read = stream.read(&mut byte).map_err(|err| err.to_string())?;
    let Some(next) = byte.first().copied() else {
      break;
    };
    if read == 0 || next == b'\n' {
      break;
    }
    buf.push(next);
  }
  String::from_utf8(buf).map_err(|err| err.to_string())
}

/// The spool path a client writes before `replace`. Exposed for tests.
pub(crate) fn spool_path(root: &Path, token: &str) -> Result<std::path::PathBuf, String> {
  let safe = !token.is_empty()
    && token
      .chars()
      .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
  if !safe {
    return Err("spool token rejected".to_owned());
  }
  Ok(root.join("spool").join(format!("{token}.json")))
}

#[cfg(test)]
#[path = "tests/client_test.rs"]
mod tests;
