//! `GET /api/v1/metrics[?epic=<id>&since=<date>]` (#451): the metrics
//! document of `toolu epic metrics --json`, folded from the journal outside
//! the world lock and kept for 60 s per query, at most 16 queries.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use toolu_runtime::config::secrets::{self, Secrets};
use toolu_state::time::epoch_millis;

use crate::http_response::{READ_METHODS, Response};
use crate::metrics::{self, Query};
use crate::paths::Paths;

/// How long one query's document is reused.
const FRESH: Duration = Duration::from_secs(60);
/// How many distinct queries are kept.
const ENTRIES: usize = 16;

/// Recent metrics bodies by query.
pub(crate) struct MetricsCache {
  entries: Mutex<VecDeque<Entry>>,
}

struct Entry {
  key: (Option<String>, Option<u64>),
  at: Instant,
  body: Vec<u8>,
}

impl MetricsCache {
  /// An empty cache for one listener.
  pub(crate) fn new() -> Self {
    Self {
      entries: Mutex::new(VecDeque::new()),
    }
  }

  /// The response to `query` (the URL's query string).
  pub(crate) fn response(&self, paths: &Paths, query: &str, secrets: &Secrets) -> Response {
    let Ok(key) = parse(query) else {
      return Response::error(400, "bad_query");
    };
    let Ok(mut entries) = self.entries.lock() else {
      return Response::error(500, "status_unavailable");
    };
    entries.retain(|entry| entry.at.elapsed() < FRESH);
    if let Some(entry) = entries.iter().find(|entry| entry.key == key) {
      return ok(entry.body.clone());
    }
    let query = Query {
      epic: key.0.clone(),
      since_ms: key.1,
      now_ms: epoch_millis(SystemTime::now()),
    };
    let body = match metrics::document(&paths.journal_dir(), &query) {
      Ok(doc) => secrets::redact_json(&doc, secrets).to_string().into_bytes(),
      Err(_err) => return Response::error(500, "status_unavailable"),
    };
    if entries.len() >= ENTRIES {
      entries.pop_front();
    }
    entries.push_back(Entry {
      key,
      at: Instant::now(),
      body: body.clone(),
    });
    ok(body)
  }
}

/// `epic` and `since` of a query string; any other key is a bad query.
fn parse(query: &str) -> Result<(Option<String>, Option<u64>), String> {
  let mut epic = None;
  let mut since = None;
  for pair in query.split('&').filter(|pair| !pair.is_empty()) {
    match pair.split_once('=') {
      Some(("epic", value)) if !value.is_empty() => epic = Some(value.to_owned()),
      Some(("since", value)) => since = Some(metrics::parse_since(value)?),
      _ => return Err(format!("bad query field {pair:?}")),
    }
  }
  Ok((epic, since))
}

fn ok(body: Vec<u8>) -> Response {
  Response {
    status: 200,
    content_type: "application/json; charset=utf-8",
    body,
    head: false,
    close: false,
    allow: READ_METHODS,
    cache: "no-store",
  }
}

#[cfg(test)]
#[path = "tests/http_metrics_test.rs"]
mod tests;
