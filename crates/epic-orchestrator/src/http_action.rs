//! Token-gated control actions: `POST /api/v1/actions/{op}` forwards one
//! allow-listed op to the state loop, which stays the only writer. Every
//! request needs the bearer token, a same-origin `Origin` and JSON, even on
//! loopback, because `answer` types into agents that run with approval bypass.

use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use serde_json::{Map, Value, json};
use toolu_runtime::config::secrets::Secrets;

use crate::http_access::authorized;
use crate::http_request::Request;
use crate::http_response::Response;
use crate::socket_loop::Msg;

/// The path prefix of every action.
pub(crate) const PREFIX: &str = "/api/v1/actions/";

/// How long an action waits for the state loop in a running engine: past
/// herdr's 30 s prompt timeout, so a slow `answer` gets its own reply
/// (`delivered: uncertain`) rather than `503` and a retry that types it twice.
pub(crate) const DEADLINE: Duration = Duration::from_secs(35);

/// The longest `answer` text, in characters.
const TEXT_LIMIT: usize = 4_000;

/// Where an action goes and how long it may wait.
pub(crate) struct Control<'a> {
  pub sender: Option<&'a Sender<Msg>>,
  pub deadline: Duration,
}

/// Gate, validate and forward one action; the reply never echoes a credential.
pub(crate) fn respond(
  request: &Request,
  op: &str,
  secrets: &Secrets,
  control: &Control<'_>,
) -> Response {
  match gate(request, secrets).and_then(|()| socket_request(op, &request.body)) {
    Ok(line) => forward(control, line),
    Err(response) => response,
  }
}

fn gate(request: &Request, secrets: &Secrets) -> Result<(), Response> {
  if request.method != "POST" {
    let mut response = Response::error(405, "method_not_allowed");
    response.allow = "POST";
    return Err(response);
  }
  if secrets.status_token().is_none() {
    return Err(Response::error(403, "control_disabled"));
  }
  if !authorized(secrets, request.authorization.as_deref()) {
    return Err(Response::error(401, "unauthorized"));
  }
  // `Host` was already checked against the bound address.
  if request.origin.as_deref() != Some(format!("http://{}", request.host).as_str()) {
    return Err(Response::error(403, "foreign_origin"));
  }
  let json = request
    .content_type
    .as_deref()
    .and_then(|value| value.split(';').next())
    .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"));
  if !json {
    return Err(Response::error(415, "unsupported_media_type"));
  }
  Ok(())
}

/// The control-socket request for `op`, signed `by: web`.
fn socket_request(op: &str, body: &[u8]) -> Result<Value, Response> {
  if !matches!(
    op,
    "pause" | "resume" | "ack" | "answer" | "release" | "host-pause" | "host-resume"
  ) {
    return Err(Response::error(404, "unknown_op"));
  }
  let invalid = || Response::error(400, "invalid_request");
  let Ok(Value::Object(body)) = serde_json::from_slice::<Value>(body) else {
    return Err(invalid());
  };
  let mut line = Map::new();
  line.insert("op".to_owned(), json!(op));
  line.insert("by".to_owned(), json!("web"));
  for (name, value) in fields(op, &body).ok_or_else(invalid)? {
    line.insert(name.to_owned(), Value::String(value));
  }
  Ok(Value::Object(line))
}

/// The validated fields `op` forwards; `None` when one is missing or invalid.
fn fields(op: &str, body: &Map<String, Value>) -> Option<Vec<(&'static str, String)>> {
  let field = |name: &'static str| key_field(body, name).ok();
  Some(match op {
    "pause" | "resume" => field("epic")?
      .map(|epic| vec![("epic", epic)])
      .unwrap_or_default(),
    // A keyless attention item is acknowledged by its kind, as the CLI does.
    "ack" => match (field("key")?, field("kind")?) {
      (Some(key), None) | (None, Some(key)) => vec![("key", key)],
      _ => return None,
    },
    "answer" => {
      let text = body
        .get("text")
        .and_then(Value::as_str)
        .filter(|text| valid_text(text))?;
      vec![("key", field("key")??), ("text", text.to_owned())]
    }
    "host-pause" | "host-resume" => {
      let host = body
        .get("host")
        .and_then(Value::as_str)
        .and_then(crate::hosts::parse_host_kind)?;
      vec![("host", host.name().to_owned())]
    }
    _ => vec![("key", field("key")??)],
  })
}

/// A field that is present but not a valid key.
struct InvalidKey;

/// An absent field is `Ok(None)`; a present one must be a valid key.
fn key_field(body: &Map<String, Value>, name: &str) -> Result<Option<String>, InvalidKey> {
  match body.get(name) {
    None => Ok(None),
    Some(Value::String(key)) if valid_key(key) => Ok(Some(key.clone())),
    Some(_) => Err(InvalidKey),
  }
}

fn valid_key(key: &str) -> bool {
  (1..=128).contains(&key.len())
    && key
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_text(text: &str) -> bool {
  (1..=TEXT_LIMIT).contains(&text.chars().count())
    && text
      .chars()
      .all(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
}

/// Send once and wait up to the deadline; a busy engine is never retried.
fn forward(control: &Control<'_>, request: Value) -> Response {
  let busy = || Response::error(503, "engine_busy");
  let Some(sender) = control.sender else {
    return busy();
  };
  let (reply, answer) = mpsc::channel();
  if sender.send(Msg::Line { request, reply }).is_err() {
    return busy();
  }
  let Ok(value) = answer.recv_timeout(control.deadline) else {
    return busy();
  };
  let refused = value.get("error").cloned();
  let (status, body, close) = match refused {
    Some(error) => (409, json!({"schema": 1, "error": error}), true),
    None => (200, value, false),
  };
  Response {
    status,
    content_type: "application/json; charset=utf-8",
    body: body.to_string().into_bytes(),
    head: false,
    close,
    allow: "POST",
    cache: "no-store",
  }
}

#[cfg(test)]
#[path = "tests/http_action_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/http_action_ops_test.rs"]
mod ops_tests;
