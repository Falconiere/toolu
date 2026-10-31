//! pr-babysit's side of the epic engine's [`BabysitTick`]: one scheduled check
//! of a pull request, reported as a [`TickReport`] whose prompt names the new
//! work by id and location only, never by reviewer prose.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde_json::Value;
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickDecision, TickReport, TickRequest};
use toolu_runtime::env::Env;
use toolu_state::time::iso_seconds;

use crate::error::Failure;
use crate::json::{array, text};
use crate::tick::{self, Source};

/// The most items a prompt lists.
const PROMPT_ITEMS: usize = 10;

/// pr-babysit's [`BabysitTick`] against one GitHub API and environment. It
/// keeps its client, so the token is read once (and again on a `401`).
#[derive(Debug)]
pub struct GitHubTick {
  config: toolu_github::Config,
  env: Env,
  client: Mutex<Option<Arc<toolu_github::Client>>>,
}

impl GitHubTick {
  /// A tick that reads GitHub through `config` with `env`'s token and proxy.
  pub fn new(config: toolu_github::Config, env: Env) -> Self {
    GitHubTick {
      config,
      env,
      client: Mutex::new(None),
    }
  }
}

impl BabysitTick for GitHubTick {
  fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError> {
    let source = Source::GitHub {
      config: self.config.clone(),
      page_size: 100,
      client: Some(&self.client),
    };
    tick(request, source, &self.env)
  }
}

/// `success`; with new attention `escalate` or `Act`; else keep going, so an
/// escalation is reported once.
fn decision(result: &Value) -> TickDecision {
  let new = result.pointer("/attention/new") == Some(&Value::Bool(true));
  match result.get("decision").and_then(Value::as_str) {
    Some("success") => TickDecision::Success,
    _ if !new => TickDecision::KeepGoing,
    Some("escalate") => TickDecision::Escalate,
    _ => TickDecision::Act,
  }
}

/// One line per actionable thread, conversation comment and review. A path is
/// the PR author's text, so its control characters are escaped.
fn items(result: &Value) -> Vec<String> {
  let class = |item: &Value| text(item, "authorClass").to_owned();
  let id = |item: &Value| item.get("id").map(ToString::to_string).unwrap_or_default();
  let mut lines: Vec<String> = array(result.pointer("/threads/actionable"))
    .iter()
    .map(|thread| {
      let line = thread
        .get("line")
        .filter(|line| !line.is_null())
        .map(ToString::to_string);
      let at = format!(
        "{}:{}",
        text(thread, "path").escape_debug(),
        line.as_deref().unwrap_or("?")
      );
      format!("- thread {} {at} ({})", text(thread, "id"), class(thread))
    })
    .collect();
  for (kind, pointer) in [
    ("conversation", "/conversation/actionable"),
    ("review", "/reviews/actionable"),
  ] {
    lines.extend(
      array(result.pointer(pointer))
        .iter()
        .map(|item| format!("- {kind} {} ({})", id(item), class(item))),
    );
  }
  lines
}

/// The worker's prompt for `request`'s new attention in `result`.
fn prompt(request: &TickRequest, result: &Value) -> String {
  let codes: Vec<&str> = array(result.pointer("/attention/codes"))
    .iter()
    .filter_map(Value::as_str)
    .collect();
  let mut out = format!(
    "pr-babysit: {}#{} needs you ({}). Tick result: {}.\n",
    request.repo,
    request.number,
    codes.join(", "),
    text(result, "resultPath"),
  );
  let items = items(result);
  for line in items.iter().take(PROMPT_ITEMS) {
    out.push_str(line);
    out.push('\n');
  }
  let more = items.len().saturating_sub(PROMPT_ITEMS);
  let more = if more > 0 {
    format!("… and {more} more\n")
  } else {
    String::new()
  };
  out.push_str(&more);
  out.push_str("Act with toolu babysit reply|resolve|record --state-file ");
  out.push_str(text(result, "statePath"));
  out.push_str("; push fixes; do not re-fetch.");
  out
}

/// `failure` as the engine's error: a GitHub rate limit keeps its wait.
fn link_error(failure: &Failure) -> LinkError {
  let extra = |key: &str| failure.extra.get(key).and_then(Value::as_u64);
  if failure.extra.get("class").and_then(Value::as_str) == Some("rate_limited") {
    return LinkError::RateLimited {
      retry_after: extra("retryAfter").map(Duration::from_secs),
      remaining: extra("remaining"),
      reset_at: extra("resetAt"),
    };
  }
  LinkError::Failed(format!("{}: {}", failure.code, failure.message))
}

/// One scheduled check of `request`'s pull request, read from `source`.
pub(crate) fn tick(
  request: &TickRequest,
  source: Source<'_>,
  env: &Env,
) -> Result<TickReport, LinkError> {
  let now = request
    .now
    .clone()
    .unwrap_or_else(|| iso_seconds(SystemTime::now()));
  let run = tick::Request {
    repo: &request.repo,
    number: request.number,
    state_file: Path::new(&request.state_file),
    snapshot_out: None,
    now: &now,
    scheduled: true,
  };
  let ticked = tick::tick(&run, source, env).map_err(|failure| link_error(&failure))?;
  let decision = decision(&ticked.result);
  let prompt = matches!(decision, TickDecision::Act | TickDecision::Escalate)
    .then(|| prompt(request, &ticked.result));
  Ok(TickReport {
    decision,
    result: ticked.result,
    graphql: ticked.graphql,
    prompt,
  })
}

#[cfg(test)]
#[path = "tests/link_test.rs"]
mod tests;
