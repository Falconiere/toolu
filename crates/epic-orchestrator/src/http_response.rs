//! No-store HTTP response encoding.

use std::io::{self, Write as _};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;
use toolu_runtime::config::secrets::Secrets;

use crate::model::World;
use crate::paths::Paths;
use crate::status_api;

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// Briefly reuse an overview while its world sequence is unchanged.
pub(crate) struct OverviewCache {
  current: Mutex<Option<CachedOverview>>,
}

struct CachedOverview {
  seq: u64,
  at: Instant,
  body: Vec<u8>,
}

impl OverviewCache {
  /// Create an empty cache for one listener.
  pub(crate) fn new() -> Self {
    Self {
      current: Mutex::new(None),
    }
  }

  /// Return the current overview or a recent response for the same sequence.
  pub(crate) fn response(&self, world: &World, paths: &Paths, secrets: &Secrets) -> Response {
    let Ok(mut current) = self.current.lock() else {
      return Response::error(500, "status_unavailable");
    };
    if let Some(entry) = current.as_ref()
      && entry.seq == world.seq
      && entry.at.elapsed() < Duration::from_millis(100)
    {
      return Response::json(entry.body.clone());
    }
    let value = match status_api::document(world, paths, None, secrets) {
      Ok(value) => value,
      Err(_err) => return Response::error(500, "status_unavailable"),
    };
    let body = value.to_string().into_bytes();
    *current = Some(CachedOverview {
      seq: world.seq,
      at: Instant::now(),
      body: body.clone(),
    });
    Response::json(body)
  }
}

/// The `Allow` value of a read-only path.
pub(crate) const READ_METHODS: &str = "GET, HEAD";

/// One finite response.
pub(crate) struct Response {
  pub status: u16,
  pub content_type: &'static str,
  pub body: Vec<u8>,
  pub head: bool,
  pub close: bool,
  /// The methods a `405` names in `Allow`.
  pub allow: &'static str,
  pub cache: &'static str,
}

impl Response {
  /// A `200` JSON body the client must not cache.
  pub(crate) fn json(body: Vec<u8>) -> Self {
    Self {
      status: 200,
      content_type: "application/json; charset=utf-8",
      body,
      head: false,
      close: false,
      allow: READ_METHODS,
      cache: "no-store",
    }
  }

  /// A fixed failure body with no request or source bytes.
  pub(crate) fn error(status: u16, code: &'static str) -> Self {
    Self {
      status,
      content_type: "application/json; charset=utf-8",
      body: json!({"schema": 1, "error": code}).to_string().into_bytes(),
      head: false,
      close: true,
      allow: READ_METHODS,
      cache: "no-store",
    }
  }
}

/// A JSON document as `200`, `None` as `404` and a read failure as `500`.
pub(crate) fn json_response(document: Result<Option<serde_json::Value>, String>) -> Response {
  match document {
    Ok(Some(value)) => Response::json(value.to_string().into_bytes()),
    Ok(None) => Response::error(404, "not_found"),
    Err(_err) => Response::error(500, "status_unavailable"),
  }
}

/// Send headers and the optional body.
///
/// # Errors
/// The peer closed or a write failed.
pub(crate) fn send(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
  let reason = reason(response.status);
  let connection = if response.close {
    "close"
  } else {
    "keep-alive"
  };
  let headers = format!(
    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: {}\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: {CSP}\r\nConnection: {}\r\n{}\r\n",
    response.status,
    reason,
    response.content_type,
    response.body.len(),
    response.cache,
    connection,
    if response.status == 405 {
      format!("Allow: {}\r\n", response.allow)
    } else {
      String::new()
    },
  );
  stream.write_all(headers.as_bytes())?;
  if !response.head {
    stream.write_all(&response.body)?;
  }
  stream.flush()
}

/// Start an SSE response; its body is written by the stream loop.
///
/// # Errors
/// The peer closed or a write failed.
pub(crate) fn stream_headers(stream: &mut TcpStream) -> io::Result<()> {
  stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: {CSP}\r\nConnection: keep-alive\r\n\r\n").as_bytes())?;
  stream.flush()
}

fn reason(status: u16) -> &'static str {
  match status {
    200 => "OK",
    400 => "Bad Request",
    401 => "Unauthorized",
    403 => "Forbidden",
    404 => "Not Found",
    405 => "Method Not Allowed",
    409 => "Conflict",
    413 => "Content Too Large",
    415 => "Unsupported Media Type",
    429 => "Too Many Requests",
    502 => "Bad Gateway",
    503 => "Service Unavailable",
    _ => "Internal Server Error",
  }
}

#[cfg(test)]
#[path = "tests/http_response_test.rs"]
mod tests;
