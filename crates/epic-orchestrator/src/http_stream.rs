//! Replayable server-sent journal events.

use std::io::Write as _;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;

use crate::model::World;
use crate::paths::Paths;
use crate::status_events;

/// Validated journal cursor and page size.
pub(crate) struct Cursor {
  since: u64,
  limit: usize,
}

/// Parse numeric query fields, with the reconnect header taking precedence.
pub(crate) fn cursor(query: &str, last_event_id: Option<&str>) -> Result<Cursor, String> {
  let mut since = None;
  let mut limit = None;
  for pair in query.split('&').filter(|pair| !pair.is_empty()) {
    let (key, value) = pair.split_once('=').ok_or("bad cursor")?;
    match key {
      "since" if since.is_none() => since = Some(value.parse::<u64>().map_err(|_err| "bad since")?),
      "limit" if limit.is_none() => {
        limit = Some(value.parse::<usize>().map_err(|_err| "bad limit")?);
      }
      _ => return Err("bad cursor".to_owned()),
    }
  }
  let since = last_event_id.map_or_else(
    || Ok(since.unwrap_or(0)),
    |last| {
      last
        .parse::<u64>()
        .map_err(|_err| "bad Last-Event-ID".to_owned())
    },
  )?;
  let limit = limit.unwrap_or(100);
  if !(1..=500).contains(&limit) {
    return Err("limit out of range".to_owned());
  }
  Ok(Cursor { since, limit })
}

/// One versioned journal page.
///
/// # Errors
/// The query is invalid or the retained journal cannot be read.
pub(crate) fn page(
  paths: &Paths,
  query: &str,
  last_event_id: Option<&str>,
  secrets: &Secrets,
) -> Result<Value, String> {
  let cursor = cursor(query, last_event_id)?;
  status_events::page(&paths.journal_dir(), cursor.since, cursor.limit, secrets)
}

/// Borrowed state needed by a replaying event stream.
pub(crate) struct Feed<'a> {
  pub paths: &'a Paths,
  pub secrets: &'a Secrets,
  pub world: &'a Arc<Mutex<World>>,
  pub changed: &'a Arc<Condvar>,
  pub stop: &'a Arc<AtomicBool>,
}

/// Send retained events and then follow the journal until disconnect or stop.
pub(crate) fn serve(stream: &mut TcpStream, mut cursor: Cursor, feed: &Feed<'_>) {
  let mut heartbeat = Instant::now();
  while !feed.stop.load(Ordering::Relaxed) {
    let page = match status_events::page(
      &feed.paths.journal_dir(),
      cursor.since,
      cursor.limit,
      feed.secrets,
    ) {
      Ok(page) => page,
      Err(_err) => {
        let _sent = stream
          .write_all(b"event: error\ndata: {\"schema\":1,\"error\":\"journal_unavailable\"}\n\n");
        return;
      }
    };
    if let Some(oldest) = page.get("oldest_seq").and_then(Value::as_u64)
      && cursor.since.saturating_add(1) < oldest
    {
      let reset = format!(
        "event: reset\ndata: {}\n\n",
        json!({"schema": 1, "oldest_seq": oldest})
      );
      if stream.write_all(reset.as_bytes()).is_err() {
        return;
      }
      cursor.since = oldest.saturating_sub(1);
      continue;
    }
    let rows = page.get("events").and_then(Value::as_array);
    if rows.is_some_and(|rows| !rows.is_empty()) {
      if !send_rows(stream, rows.into_iter().flatten(), &mut cursor.since) {
        return;
      }
      heartbeat = Instant::now();
      continue;
    }
    if heartbeat.elapsed() >= Duration::from_secs(15) {
      if stream.write_all(b": heartbeat\n\n").is_err() {
        return;
      }
      heartbeat = Instant::now();
    }
    if !wait_for_change(stream, feed) {
      return;
    }
  }
}

fn wait_for_change(stream: &mut TcpStream, feed: &Feed<'_>) -> bool {
  let Ok(guard) = feed.world.lock() else {
    send_status_error(stream);
    return false;
  };
  if feed
    .changed
    .wait_timeout(guard, Duration::from_secs(1))
    .is_err()
  {
    send_status_error(stream);
    return false;
  }
  true
}

fn send_status_error(stream: &mut TcpStream) {
  let _sent =
    stream.write_all(b"event: error\ndata: {\"schema\":1,\"error\":\"status_unavailable\"}\n\n");
}

fn send_rows<'a>(
  stream: &mut TcpStream,
  rows: impl Iterator<Item = &'a Value>,
  since: &mut u64,
) -> bool {
  for row in rows {
    let Some(seq) = row.get("seq").and_then(Value::as_u64) else {
      continue;
    };
    let message = format!("id: {seq}\nevent: journal\ndata: {row}\n\n");
    if stream.write_all(message.as_bytes()).is_err() {
      return false;
    }
    *since = seq;
  }
  true
}

#[cfg(test)]
#[path = "tests/http_stream_test.rs"]
mod tests;
