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
      return Response {
        status: 200,
        content_type: "application/json; charset=utf-8",
        body: entry.body.clone(),
        head: false,
        close: false,
      };
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
    Response {
      status: 200,
      content_type: "application/json; charset=utf-8",
      body,
      head: false,
      close: false,
    }
  }
}

/// One finite response to a GET or HEAD request.
pub(crate) struct Response {
  pub status: u16,
  pub content_type: &'static str,
  pub body: Vec<u8>,
  pub head: bool,
  pub close: bool,
}

impl Response {
  /// A fixed failure body with no request or source bytes.
  pub(crate) fn error(status: u16, code: &'static str) -> Self {
    Self {
      status,
      content_type: "application/json; charset=utf-8",
      body: json!({"schema": 1, "error": code}).to_string().into_bytes(),
      head: false,
      close: true,
    }
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
    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'\r\nConnection: {}\r\n{}\r\n",
    response.status,
    reason,
    response.content_type,
    response.body.len(),
    connection,
    if response.status == 405 {
      "Allow: GET, HEAD\r\n"
    } else {
      ""
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
  stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n")?;
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
    413 => "Content Too Large",
    429 => "Too Many Requests",
    503 => "Service Unavailable",
    _ => "Internal Server Error",
  }
}

#[cfg(test)]
#[path = "tests/http_response_test.rs"]
mod tests;
