//! One GitHub call: the URL, the headers, and the attempts the retry policy
//! allows, with one token re-read on a `401`.

use std::sync::MutexGuard;
use std::thread;
use std::time::Duration;

use serde_json::Value;
use toolu_http::{Auth, Method, Request, Response};

use crate::policy::{self, Class};
use crate::token::{self, Token, Tokens};
use crate::{Client, Error};

/// The `User-Agent` GitHub requires on every request.
const USER_AGENT: &str = concat!("toolu/", env!("CARGO_PKG_VERSION"));

/// One request before its attempts.
pub(crate) struct Call<'a> {
  /// The method.
  pub(crate) method: Method,
  /// A path under the API, or a URL that starts with it.
  pub(crate) path: &'a str,
  /// A JSON body.
  pub(crate) body: Option<Vec<u8>>,
  /// The `ETag` of the caller's cached copy.
  pub(crate) etag: Option<&'a str>,
}

/// What an attempt leaves the call to do.
enum Step {
  /// Return this response.
  Done(Response),
  /// Try again after this wait.
  Wait(Duration),
}

impl Client {
  /// Send `call` until it succeeds or the policy gives up: its final response
  /// and the attempts used.
  pub(crate) fn call(&self, call: &Call<'_>) -> Result<(Response, u32), Error> {
    let url = self.url(call.path)?;
    let headers = headers(call)?;
    let (mut attempt, mut reread) = (0, false);
    loop {
      attempt += 1;
      let sent = self.tokens().current();
      let auth = Auth::Bearer(sent.expose().to_owned());
      let outcome = self.http.send(&Request {
        method: call.method,
        url: &url,
        auth: &auth,
        headers: &headers,
        body: call.body.as_deref(),
      });
      let class = policy::classify(&outcome);
      if class == Class::Unauthorized && !std::mem::replace(&mut reread, true) && self.reread(&sent)
      {
        attempt -= 1;
        continue;
      }
      match self.step(outcome, class, attempt)? {
        Step::Done(response) => return Ok((response, attempt)),
        Step::Wait(wait) => thread::sleep(wait),
      }
    }
  }

  /// What attempt `attempt`, classed `class`, leaves the call to do.
  fn step(
    &self,
    outcome: Result<Response, toolu_http::Error>,
    class: Class,
    attempt: u32,
  ) -> Result<Step, Error> {
    let retry = &self.config.retry;
    match (class, outcome) {
      (Class::Success, Ok(response)) => Ok(Step::Done(response)),
      (Class::Unauthorized, _) => Err(Error::Unauthorized),
      (Class::RateLimited { wait }, outcome) => {
        let pause = wait.unwrap_or_else(|| retry.backoff(attempt));
        if pause > retry.max_wait || attempt >= retry.attempts {
          let status = outcome.map_or(0, |response| response.status);
          return Err(Error::RateLimited {
            status,
            retry_after: wait,
          });
        }
        Ok(Step::Wait(pause))
      }
      (Class::Transient, _) if attempt < retry.attempts => Ok(Step::Wait(retry.backoff(attempt))),
      (Class::Success | Class::Transient | Class::Permanent, outcome) => Err(self.failure(outcome)),
    }
  }

  /// The error a final failed attempt leaves.
  fn failure(&self, outcome: Result<Response, toolu_http::Error>) -> Error {
    match outcome {
      Ok(response) => Error::Status {
        status: response.status,
        message: self.message(&response.body),
      },
      Err(err) => Error::Transport(err),
    }
  }

  /// A JSON error body's `message`, redacted, or empty.
  pub(crate) fn message(&self, body: &[u8]) -> String {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    let message = parsed
      .as_ref()
      .and_then(|value| value.get("message"))
      .and_then(Value::as_str)
      .map_or_else(String::new, str::to_owned);
    self.tokens().redact(&message)
  }

  /// The URL for `path`: a `/` path is appended to the API URL, and a URL that
  /// starts with the API URL and `/` is used as is.
  pub(crate) fn url(&self, path: &str) -> Result<String, Error> {
    let api = &self.config.api_url;
    let url = if path.starts_with('/') && !path.starts_with("//") {
      format!("{api}{path}")
    } else if path
      .strip_prefix(api.as_str())
      .is_some_and(|rest| rest.starts_with('/'))
    {
      path.to_owned()
    } else {
      return Err(Error::Config(format!("{path} is not a path under {api}")));
    };
    // Refused once here, a URL no attempt could send is never retried. The
    // URI parser accepts non-ASCII, which GitHub paths must percent-encode.
    let ascii = path.chars().all(|ch| ch.is_ascii_graphic());
    toolu_http::check_url(&url)
      .ok()
      .filter(|()| ascii)
      .ok_or_else(|| {
        Error::Config(format!(
          "{path:?} is not a URL path: percent-encode spaces, controls, non-ASCII and \
         reserved characters"
        ))
      })?;
    Ok(url)
  }

  /// The token store, recovered if a panicking thread poisoned it.
  pub(crate) fn tokens(&self) -> MutexGuard<'_, Tokens> {
    match self.tokens.lock() {
      Ok(tokens) => tokens,
      Err(poisoned) => poisoned.into_inner(),
    }
  }

  /// Read the token again after a `401` to `sent`: whether the token to retry
  /// with differs from `sent`. A failed read is no new token. The lock is not
  /// held while `gh` runs.
  fn reread(&self, sent: &Token) -> bool {
    match token::resolve(&self.env) {
      Ok(fresh) => self.tokens().refresh(sent, fresh),
      Err(_unreadable) => false,
    }
  }
}

/// The request headers for `call`.
fn headers<'a>(call: &Call<'a>) -> Result<Vec<(&'a str, &'a str)>, Error> {
  let mut headers = vec![
    ("accept", "application/vnd.github+json"),
    ("x-github-api-version", "2022-11-28"),
    ("user-agent", USER_AGENT),
  ];
  if let Some(etag) = call.etag {
    if !etag.chars().all(|ch| ch.is_ascii_graphic()) {
      return Err(Error::Config(format!("{etag:?} is not an ETag")));
    }
    headers.push(("if-none-match", etag));
  }
  if call.body.is_some() {
    headers.push(("content-type", "application/json"));
  }
  Ok(headers)
}

#[cfg(test)]
#[path = "tests/call_test.rs"]
mod tests;
