//! Local, token-gated reads of a worker's visible herdr screen.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use toolu_runtime::config::secrets::{Secrets, redact_text};

use crate::http_live::{Herdr, Limiter, request};
use crate::http_response::{Response, json_response};
use crate::model::World;

const FRESH: Duration = Duration::from_secs(1);
const MAX_BODY: usize = 64 * 1024;

#[derive(Clone)]
struct Sample {
  at: Instant,
  sampled_at: String,
  target: String,
  result: Result<String, String>,
}

/// One cache per key, backed by the listener's shared herdr limiter.
pub(crate) struct Screen {
  herdr: Herdr,
  session: Option<String>,
  cached: Mutex<BTreeMap<String, Arc<Mutex<Option<Sample>>>>>,
}

impl Screen {
  /// Build the screen cache for this listener's herdr source.
  pub(crate) fn new(herdr: Herdr, session: Option<String>) -> Self {
    Self {
      herdr,
      session,
      cached: Mutex::new(BTreeMap::new()),
    }
  }

  /// Read one worker screen after the HTTP route has checked its bearer token.
  pub(crate) fn response(
    &self,
    key: &str,
    limiter: &Limiter,
    world: &Mutex<World>,
    secrets: &Secrets,
  ) -> Response {
    let (target, status) = match world.lock() {
      Ok(world) => match world.issues.get(key) {
        Some(issue) => (issue.agent.clone(), issue.agent_status.clone()),
        None => return Response::error(404, "not_found"),
      },
      Err(_err) => return Response::error(500, "status_unavailable"),
    };
    let socket = match &self.herdr {
      Herdr::Off => return state(key, "off", secrets),
      Herdr::Scripted => return state(key, "scripted", secrets),
      Herdr::Socket(socket) => socket,
    };
    if target.is_empty() {
      return not_running(key, secrets);
    }
    let sample = match self.sample(key, &target, socket, limiter) {
      Ok(sample) => sample,
      Err(response) => return response,
    };
    match &sample.result {
      Ok(_raw) => self.render(key, &status, &sample, secrets),
      Err(reason)
        if matches!(
          reason.as_str(),
          "error: agent_not_found" | "error: pane_not_found"
        ) =>
      {
        not_running(key, secrets)
      }
      Err(reason) => state(key, reason, secrets),
    }
  }

  fn sample(
    &self,
    key: &str,
    target: &str,
    socket: &Path,
    limiter: &Limiter,
  ) -> Result<Sample, Response> {
    let cell = {
      let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
      Arc::clone(
        cached
          .entry(key.to_owned())
          .or_insert_with(|| Arc::new(Mutex::new(None))),
      )
    };
    let mut cached = cell.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(sample) = cached
      .as_ref()
      .filter(|sample| sample.target == target && sample.at.elapsed() < FRESH)
      .cloned()
    {
      return Ok(sample);
    }
    let Some(_permit) = limiter.try_acquire() else {
      return Err(Response::error(503, "herdr_busy"));
    };
    let result = read(socket, target);
    let sample = Sample {
      at: Instant::now(),
      sampled_at: toolu_state::time::iso_millis(SystemTime::now()),
      target: target.to_owned(),
      result,
    };
    *cached = Some(sample.clone());
    Ok(sample)
  }

  fn render(&self, key: &str, status: &str, sample: &Sample, secrets: &Secrets) -> Response {
    let Ok(raw) = &sample.result else {
      return Response::error(500, "screen_unavailable");
    };
    let mut text = redact_text(raw, secrets);
    loop {
      let mut value = json!({
        "schema": 1, "key": redact_text(key, secrets),
        "agent_status": status, "text": text, "lines": text.lines().count(),
        "sampled_at": sample.sampled_at,
      });
      if status == "blocked"
        && let Some(fields) = value.as_object_mut()
      {
        fields.insert(
          "attach".to_owned(),
          json!(redact_text(
            &attach(self.session.as_deref(), &sample.target),
            secrets
          )),
        );
      }
      let body = value.to_string().into_bytes();
      if body.len() <= MAX_BODY {
        return ok(body);
      }
      if text.is_empty() {
        return Response::error(500, "screen_unavailable");
      }
      // JSON escaping can cost more bytes than the raw text; keep a suffix.
      let mut cut = body
        .len()
        .saturating_sub(MAX_BODY)
        .min(text.len().div_ceil(2));
      while !text.is_char_boundary(cut) {
        cut += 1;
      }
      text = text.get(cut..).unwrap_or_default().to_owned();
    }
  }
}

fn read(socket: &Path, target: &str) -> Result<String, String> {
  let reply = request(
    socket,
    "agent.read",
    &json!({"target": target, "source": "visible", "format": "text"}),
  )?;
  reply
    .pointer("/result/read/text")
    .and_then(Value::as_str)
    .map(str::to_owned)
    .ok_or_else(|| "error: invalid_reply".to_owned())
}

fn state(key: &str, reason: &str, secrets: &Secrets) -> Response {
  json_response(Ok(Some(json!({
    "schema": 1, "key": redact_text(key, secrets), "available": false,
    "reason": redact_text(reason, secrets).chars().take(160).collect::<String>(),
  }))))
}

fn not_running(key: &str, secrets: &Secrets) -> Response {
  json_response(Ok(Some(json!({
    "schema": 1, "key": redact_text(key, secrets), "state": "not_running",
  }))))
}

fn ok(body: Vec<u8>) -> Response {
  Response {
    status: 200,
    content_type: "application/json; charset=utf-8",
    body,
    head: false,
    close: false,
    allow: crate::http_response::READ_METHODS,
    cache: "no-store",
  }
}

fn attach(session: Option<&str>, target: &str) -> String {
  let session = session
    .map(|name| format!("--session {} ", shell_word(name)))
    .unwrap_or_default();
  format!("herdr {session}agent attach {}", shell_word(target))
}

fn shell_word(value: &str) -> String {
  if !value.is_empty()
    && value
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
  {
    value.to_owned()
  } else {
    format!("'{}'", value.replace('\'', "'\\''"))
  }
}

#[cfg(test)]
#[path = "tests/http_screen_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/http_screen_concurrency_test.rs"]
mod concurrency_tests;

#[cfg(test)]
#[path = "tests/http_screen_render_test.rs"]
mod render_tests;
