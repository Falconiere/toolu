//! One request on the engine socket.

use std::io::{BufRead, BufReader, Read, Write};
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
  let mut stream = UnixStream::connect(paths.socket()).map_err(|err| format!("connect: {err}"))?;
  let hello = read_json(&mut stream)?;
  let server = hello.get("protocol").and_then(Value::as_u64).unwrap_or(0);
  if server != protocol {
    spool(paths, request)?;
    write_json(&mut stream, &json!({"op": "replace", "protocol": server}))?;
    return read_json(&mut stream);
  }
  if matches!(
    request.get("op").and_then(Value::as_str),
    Some("report" | "notify")
  ) {
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
      Err(err) if err.starts_with("connect:") => last = err,
      Err(err) => return Err(err),
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

/// The last reply line on a herdr connection, read in blocks since nothing
/// follows it, up to 16 MiB. A read deadline that passes reads as `timed out`.
///
/// # Errors
/// The read fails or times out, the line passes 16 MiB, or it is not UTF-8.
pub(crate) fn read_text(stream: &mut UnixStream) -> Result<String, String> {
  read_capped(stream, 64 * 1024, 16 * 1024 * 1024)
}

/// One line, read a byte at a time so a later line stays on the stream; up to
/// 16 MiB, as a herdr `session.snapshot` or an engine `status` can be large.
fn read_line(stream: &mut UnixStream) -> Result<String, String> {
  read_capped(stream, 1, 16 * 1024 * 1024)
}

fn read_capped(stream: &mut UnixStream, block: usize, cap: usize) -> Result<String, String> {
  let mut buf = Vec::new();
  let limit = u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1);
  BufReader::with_capacity(block, stream)
    .take(limit)
    .read_until(b'\n', &mut buf)
    .map_err(|err| {
      if matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
      ) {
        "timed out".to_owned()
      } else {
        err.to_string()
      }
    })?;
  if buf.last() == Some(&b'\n') {
    buf.pop();
  } else if buf.len() > cap {
    return Err(format!("reply line exceeds {cap} bytes"));
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
