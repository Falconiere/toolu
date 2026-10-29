//! Local, token-gated reads of one worker's background logs.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};
use toolu_engine::resources::job_logs::{self, Chunk, Entry};
use toolu_runtime::config::secrets::{Secrets, redact_text, redact_window, redaction_overlap};

use crate::http::Context;
use crate::http_access::authorized;
use crate::http_request::Request;
use crate::http_response::{Response, json_response};
use crate::model::{Issue, World};

mod claude;

use claude::{claude_logs, read_claude};

const PAGE_BYTES: usize = 64 * 1024;
const READ_BYTES: usize = 60 * 1024;

enum Source {
  Job(Entry),
  Claude(PathBuf),
}

struct Log {
  id: String,
  source: Source,
  label: String,
  state: &'static str,
  size: u64,
  started_at: String,
  updated_at: String,
}

fn route(path: &str) -> Option<(&str, Option<&str>)> {
  let rest = path.strip_prefix("/api/v1/issues/")?;
  let (key, tail) = rest.split_once("/logs")?;
  if key.is_empty() || key.contains('/') {
    return None;
  }
  match tail {
    "" => Some((key, None)),
    tail => tail.strip_prefix('/').map(|id| (key, Some(id))),
  }
}

/// Whether a path is one of the local log routes that always needs a token.
pub(crate) fn is_route(path: &str) -> bool {
  route(path).is_some()
}

/// A log listing or detail response, when `path` names this local route.
pub(crate) fn response(
  context: &Context,
  path: &str,
  query: &str,
  request: &Request,
  secrets: &Secrets,
) -> Option<Response> {
  let (key, id) = route(path)?;
  if secrets.status_token().is_none() {
    return Some(Response::error(403, "control_disabled"));
  }
  if !authorized(secrets, request.authorization.as_deref()) {
    return Some(Response::error(401, "unauthorized"));
  }
  let issue = match local_issue(&context.world, key) {
    Ok(Some(issue)) => issue,
    Ok(None) => return Some(Response::error(404, "not_found")),
    Err(()) => return Some(Response::error(500, "status_unavailable")),
  };
  let logs = match discover(&context.paths.root, &issue, secrets) {
    Ok(logs) => logs,
    Err(_err) => return Some(unavailable("log_store_unavailable")),
  };
  Some(match id {
    None => json_response(Ok(Some(json!({
      "schema": 1,
      "items": logs.iter().map(|log| summary(log, secrets)).collect::<Vec<_>>(),
    })))),
    Some(id) => detail(&logs, id, query, secrets),
  })
}

fn local_issue(world: &Mutex<World>, key: &str) -> Result<Option<Issue>, ()> {
  Ok(world.lock().map_err(|_err| ())?.issues.get(key).cloned())
}

fn summary(log: &Log, secrets: &Secrets) -> Value {
  json!({
    "id": log.id, "source": match log.source { Source::Job(_) => "job", Source::Claude(_) => "claude-task" },
    "label": redact_text(&log.label, secrets), "state": log.state,
    "size": log.size, "started_at": log.started_at, "updated_at": log.updated_at,
  })
}

fn discover(root: &Path, issue: &Issue, secrets: &Secrets) -> Result<Vec<Log>, String> {
  let mut logs = job_logs::list(root, &issue.key)?
    .into_iter()
    .map(|entry| Log {
      id: entry.id.clone(),
      label: entry.label.clone(),
      state: entry.state,
      size: entry.size,
      started_at: entry.started_at.clone(),
      updated_at: entry.updated_at.clone(),
      source: Source::Job(entry),
    })
    .collect::<Vec<_>>();
  logs.extend(claude_logs(issue, secrets)?);
  logs.sort_by(|a, b| {
    b.started_at
      .cmp(&a.started_at)
      .then_with(|| b.id.cmp(&a.id))
  });
  Ok(logs)
}

fn offset(query: &str) -> Result<Option<u64>, ()> {
  if query.is_empty() {
    return Ok(None);
  }
  query
    .strip_prefix("offset=")
    .filter(|text| !text.is_empty() && !text.contains('&'))
    .and_then(|text| text.parse().ok())
    .map(Some)
    .ok_or(())
}

fn detail(logs: &[Log], id: &str, query: &str, secrets: &Secrets) -> Response {
  let Some(log) = logs.iter().find(|log| log.id == id) else {
    return Response::error(404, "not_found");
  };
  let Ok(requested) = offset(query) else {
    return Response::error(400, "bad_offset");
  };
  let desired = requested.unwrap_or_else(|| log.size.saturating_sub(PAGE_BYTES as u64));
  let overlap = redaction_overlap(secrets);
  let start = desired.saturating_sub(overlap as u64);
  let limit = READ_BYTES.saturating_add(overlap.saturating_mul(2));
  let chunk = match &log.source {
    Source::Job(entry) => job_logs::read(entry, Some(start), limit),
    Source::Claude(path) => read_claude(path, start, limit),
  };
  match chunk {
    Ok(chunk) => bounded(&chunk, desired, requested, log.state, secrets),
    Err(_err) => unavailable("unreadable_file"),
  }
}

fn bounded(
  chunk: &Chunk,
  desired: u64,
  requested: Option<u64>,
  state: &str,
  secrets: &Secrets,
) -> Response {
  let offset = desired.clamp(chunk.offset, chunk.size);
  let start = usize::try_from(offset - chunk.offset).unwrap_or(chunk.bytes.len());
  let mut end = start.saturating_add(READ_BYTES).min(chunk.bytes.len());
  loop {
    let text = redact_window(&chunk.bytes, start, end, secrets);
    let value = json!({
      "schema": 1, "text": text, "offset": offset,
      "next_offset": offset.saturating_add(u64::try_from(end - start).unwrap_or(u64::MAX)),
      "size": chunk.size, "state": state,
      "truncated": requested.is_some_and(|requested| requested < chunk.offset),
    });
    let body = value.to_string().into_bytes();
    if body.len() <= PAGE_BYTES {
      return Response::json(body);
    }
    if end == start {
      return Response::error(500, "log_unavailable");
    }
    end = start + (end - start) / 2;
  }
}

fn unavailable(reason: &str) -> Response {
  json_response(Ok(Some(
    json!({"schema": 1, "available": false, "reason": reason}),
  )))
}

#[cfg(test)]
#[path = "tests/http_logs_test.rs"]
mod tests;
