//! Bounded HTTP/1.1 request parsing for the status listener.

use std::io::{BufReader, Read as _};
use std::net::TcpStream;

/// The largest `POST` body an action may carry.
const BODY_LIMIT: u64 = 16 * 1024;

/// The request fields used by the router.
pub(crate) struct Request {
  pub method: String,
  pub target: String,
  pub host: String,
  pub authorization: Option<String>,
  pub last_event_id: Option<String>,
  pub origin: Option<String>,
  pub content_type: Option<String>,
  pub close: bool,
  /// A `POST` body; empty for every other method.
  pub body: Vec<u8>,
}

/// Fixed errors that never include request bytes.
#[derive(Debug)]
pub(crate) enum ReadError {
  Bad,
  TooLarge,
  Io,
}

#[derive(Default)]
struct HeaderSeen {
  host: bool,
  length: Option<u64>,
}

/// One request, or `None` after a clean disconnect.
pub(crate) fn read_request(
  reader: &mut BufReader<TcpStream>,
) -> Result<Option<Request>, ReadError> {
  let Some(start_line) = line(reader, 8_192)? else {
    return Ok(None);
  };
  let mut parts = start_line.split(' ');
  let (Some(method), Some(target), Some(version), None) =
    (parts.next(), parts.next(), parts.next(), parts.next())
  else {
    return Err(ReadError::Bad);
  };
  if version != "HTTP/1.1" || !target.starts_with('/') || target.len() > 2_048 {
    return Err(ReadError::Bad);
  }
  let mut request = Request {
    method: method.to_owned(),
    target: target.to_owned(),
    host: String::new(),
    authorization: None,
    last_event_id: None,
    origin: None,
    content_type: None,
    close: false,
    body: Vec::new(),
  };
  let mut total = start_line.len() + 2;
  let mut seen = HeaderSeen::default();
  loop {
    let header = line(reader, 8_192usize.saturating_sub(total))?.ok_or(ReadError::Bad)?;
    total += header.len() + 2;
    if total > 8_192 {
      return Err(ReadError::TooLarge);
    }
    if header.is_empty() {
      break;
    }
    let (name, value) = header.split_once(':').ok_or(ReadError::Bad)?;
    accept_header(&mut request, name, value.trim(), &mut seen)?;
  }
  if request.host.is_empty() {
    return Err(ReadError::Bad);
  }
  request.body = body(reader, &request.method, seen.length)?;
  Ok(Some(request))
}

/// A `POST` needs a `Content-Length` up to [`BODY_LIMIT`]; other methods carry none.
fn body(
  reader: &mut BufReader<TcpStream>,
  method: &str,
  length: Option<u64>,
) -> Result<Vec<u8>, ReadError> {
  let size = match (method, length) {
    ("POST", None) => return Err(ReadError::Bad),
    ("POST", Some(size)) if size <= BODY_LIMIT => size,
    (_, Some(0) | None) => return Ok(Vec::new()),
    _ => return Err(ReadError::TooLarge),
  };
  let mut bytes = Vec::new();
  reader
    .by_ref()
    .take(size)
    .read_to_end(&mut bytes)
    .map_err(|_err| ReadError::Io)?;
  if u64::try_from(bytes.len()).ok() != Some(size) {
    return Err(ReadError::Io);
  }
  Ok(bytes)
}

fn accept_header(
  request: &mut Request,
  name: &str,
  value: &str,
  seen: &mut HeaderSeen,
) -> Result<(), ReadError> {
  match name.to_ascii_lowercase().as_str() {
    "host" if !seen.host => {
      value.clone_into(&mut request.host);
      seen.host = true;
    }
    "authorization" if request.authorization.is_none() => {
      request.authorization = Some(value.to_owned());
    }
    "last-event-id" if request.last_event_id.is_none() => {
      request.last_event_id = Some(value.to_owned());
    }
    "connection" => request.close = value.eq_ignore_ascii_case("close"),
    "origin" if request.origin.is_none() => request.origin = Some(value.to_owned()),
    "content-type" if request.content_type.is_none() => {
      request.content_type = Some(value.to_owned());
    }
    "content-length" if seen.length.is_none() => {
      seen.length = Some(value.parse::<u64>().map_err(|_err| ReadError::Bad)?);
    }
    "host" | "authorization" | "last-event-id" | "origin" | "content-type" | "content-length"
    | "transfer-encoding" => {
      return Err(ReadError::Bad);
    }
    _ => {}
  }
  Ok(())
}

fn line(reader: &mut BufReader<TcpStream>, limit: usize) -> Result<Option<String>, ReadError> {
  let mut bytes = Vec::new();
  loop {
    let mut byte = [0_u8; 1];
    match reader.read(&mut byte) {
      Ok(0) if bytes.is_empty() => return Ok(None),
      Ok(0) => return Err(ReadError::Bad),
      Ok(_) if byte[0] == b'\n' => {
        if bytes.pop() != Some(b'\r') {
          return Err(ReadError::Bad);
        }
        return String::from_utf8(bytes)
          .map(Some)
          .map_err(|_err| ReadError::Bad);
      }
      Ok(_) => bytes.push(byte[0]),
      Err(_err) => return Err(ReadError::Io),
    }
    if bytes.len() > limit {
      return Err(ReadError::TooLarge);
    }
  }
}

#[cfg(test)]
#[path = "tests/http_request_test.rs"]
mod tests;
