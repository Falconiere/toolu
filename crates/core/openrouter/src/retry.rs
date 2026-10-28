//! Three attempts share the caller's deadline, including waits and body reads.

use std::thread;
use std::time::{Duration, Instant};

use toolu_http::{Auth, Method, Request, Response};

use crate::{Client, Error, MAX_BODY_BYTES};

const ATTEMPTS: u32 = 3;
const MAX_RETRY_AFTER_SECS: u64 = 60;

impl Client {
  /// Send one structured chat request before `deadline`.
  ///
  /// # Errors
  /// Returns a typed configuration, transport, upstream or response error.
  pub fn complete_structured(
    &self,
    request: &crate::StructuredRequest<'_>,
    deadline: Instant,
  ) -> Result<crate::Completion, Error> {
    let body = request.encode(&self.model)?;
    let auth = Auth::Bearer(self.key.clone());
    for attempt in 1..=ATTEMPTS {
      let outcome = self.send_once(&body, &auth, deadline);
      match outcome {
        Ok(response) => {
          let failure = match answer(&response, &self.model) {
            Ok(completion) => return Ok(completion),
            Err(failure) => failure,
          };
          let delay = status_delay(&response, attempt, self.config.pause, &failure)?;
          wait(deadline, delay, failure)?;
        }
        Err(err) => {
          let (delay, failure) = transport_delay(&err, attempt, self.config.pause)?;
          wait(deadline, delay, failure)?;
        }
      }
    }
    Err(Error::Timeout)
  }

  fn send_once(
    &self,
    body: &[u8],
    auth: &Auth,
    deadline: Instant,
  ) -> Result<Response, toolu_http::Error> {
    let remaining = deadline
      .checked_duration_since(Instant::now())
      .filter(|left| !left.is_zero())
      .ok_or(toolu_http::Error::Timeout)?;
    let http = toolu_http::Client::new(
      toolu_http::Config {
        timeout: remaining,
        max_body_bytes: MAX_BODY_BYTES,
        max_redirects: 0,
        test_root_ca_der: self.config.test_root_ca_der.clone(),
      },
      &self.env,
    )?;
    http.send(&Request {
      method: Method::Post,
      url: &self.config.endpoint,
      auth,
      headers: &[
        ("content-type", "application/json"),
        ("accept", "application/json"),
      ],
      body: Some(body),
    })
  }
}

/// A 2xx completion, or the typed failure of its body or of a non-2xx status.
fn answer(response: &Response, model: &str) -> Result<crate::Completion, Error> {
  if (200..300).contains(&response.status) {
    crate::response::parse(&response.body, model)
  } else {
    Err(status(response))
  }
}

/// The wait before another attempt after a status or embedded `failure`, or
/// the failure itself when it is final.
fn status_delay(
  response: &Response,
  attempt: u32,
  pause: Duration,
  failure: &Error,
) -> Result<Duration, Error> {
  if attempt == ATTEMPTS || !transient(failure) {
    return Err(failure.clone());
  }
  retry_delay(response, attempt, pause).ok_or_else(|| failure.clone())
}

fn transport_delay(
  err: &toolu_http::Error,
  attempt: u32,
  pause: Duration,
) -> Result<(Duration, Error), Error> {
  let failure = match err {
    toolu_http::Error::Timeout => Error::Timeout,
    toolu_http::Error::Transport(_) => Error::Transport,
    toolu_http::Error::BodyTooLarge => return Err(Error::BodyTooLarge),
    toolu_http::Error::InvalidConfig(_) => return Err(Error::InvalidConfig),
    toolu_http::Error::HttpStatus(_)
    | toolu_http::Error::Encode(_)
    | toolu_http::Error::Decode(_) => return Err(Error::Transport),
  };
  if attempt == ATTEMPTS {
    return Err(failure);
  }
  Ok((pause.saturating_mul(attempt), failure))
}

fn wait(deadline: Instant, delay: Duration, failure: Error) -> Result<(), Error> {
  if !fits(deadline, delay) {
    return Err(failure);
  }
  thread::sleep(delay);
  Ok(())
}

fn retryable(status: u16) -> bool {
  status == 408 || status == 429 || status >= 500
}

fn transient(error: &Error) -> bool {
  matches!(error, Error::RateLimited { .. })
    || matches!(error, Error::Upstream(code) if retryable(*code))
}

fn status(response: &Response) -> Error {
  let doc = serde_json::from_slice::<serde_json::Value>(&response.body).ok();
  let message = doc
    .as_ref()
    .and_then(|doc| doc.get("error"))
    .and_then(|error| error.get("message"))
    .and_then(serde_json::Value::as_str);
  Error::from_status(response.status, message, retry_after(response))
}

fn retry_after(response: &Response) -> Option<u64> {
  response
    .header("retry-after")
    .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
    .and_then(|value| value.parse().ok())
}

fn retry_delay(response: &Response, attempt: u32, pause: Duration) -> Option<Duration> {
  let units = match response.header("retry-after") {
    Some(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
      value.parse::<u64>().ok()?
    }
    _ => u64::from(attempt),
  };
  if units > MAX_RETRY_AFTER_SECS {
    return None;
  }
  Some(pause.saturating_mul(u32::try_from(units).ok()?))
}

fn fits(deadline: Instant, delay: Duration) -> bool {
  deadline
    .checked_duration_since(Instant::now())
    .is_some_and(|remaining| delay < remaining)
}

#[cfg(test)]
#[path = "tests/retry_test.rs"]
mod tests;
