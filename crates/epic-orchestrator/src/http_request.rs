//! Bounded HTTP/1.1 request parsing for the status listener.

use std::io::{BufReader, Read as _};
use std::net::TcpStream;

/// The request fields used by the read-only router.
pub(crate) struct Request {
  pub method: String,
  pub target: String,
  pub host: String,
  pub authorization: Option<String>,
  pub last_event_id: Option<String>,
  pub close: bool,
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
  length: bool,
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
    close: false,
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
  Ok(Some(request))
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
    "content-length" if !seen.length => {
      seen.length = true;
      let size = value.parse::<u64>().map_err(|_err| ReadError::Bad)?;
      if size > 0 {
        return Err(ReadError::TooLarge);
      }
    }
    "host" | "authorization" | "last-event-id" | "content-length" | "transfer-encoding" => {
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
