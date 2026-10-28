//! `jev.ts`'s attempts: three, with `retry-after` or exponential pauses for
//! 408, 429 and 5xx answers and for transport failures.

use std::thread;
use std::time::{Duration, Instant};

use toolu_http::{Auth, Method, Request, Response};

use crate::{Error, Jev, http_client};

/// Attempts per call.
const ATTEMPTS: u32 = 3;

/// The longest wait, in pause units, before a status is returned instead.
const MAX_UNITS: u32 = 60;

/// Whether `status` is worth another attempt.
pub(crate) fn retryable(status: u16) -> bool {
  status == 408 || status == 429 || status >= 500
}

/// The pause units before the attempt after `attempt` (1-based); `None` when
/// the status is returned instead.
pub(crate) fn delay(response: &Response, attempt: u32) -> Option<u32> {
  let base = 1_u32 << attempt.saturating_sub(1).min(16);
  let digits = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
  let value = match (
    response.header("retry-after"),
    response.header("retry-after-ms"),
  ) {
    (Some(seconds), _) if digits(seconds) => {
      if seconds.len() > 2 {
        return None;
      }
      seconds.parse::<u32>().ok()?
    }
    (_, Some(millis)) if digits(millis) => {
      if millis.len() > 8 {
        return None;
      }
      millis.parse::<u32>().ok()?.div_ceil(1000)
    }
    _ => base,
  };
  (value <= MAX_UNITS).then_some(value.max(base))
}

impl Jev {
  /// POST `body` with up to three attempts: the 2xx body, or the last failure.
  /// With a `deadline`, no attempt outlives it and no pause runs past it.
  pub(crate) fn post(&self, body: &[u8], deadline: Option<Instant>) -> Result<String, Error> {
    let auth = Auth::Bearer(self.key.0.clone());
    let mut attempt = 1;
    loop {
      let (units, last) = match self.send(body, &auth, deadline) {
        Ok(response) if (200..300).contains(&response.status) => {
          return Ok(String::from_utf8_lossy(&response.body).into_owned());
        }
        Ok(response) => (
          self.retry_status(&response, attempt)?,
          self.http_error(&response),
        ),
        Err(err @ (toolu_http::Error::Timeout | toolu_http::Error::Transport(_)))
          if attempt < ATTEMPTS =>
        {
          (1_u32 << (attempt - 1), failure(err))
        }
        Err(err) => return Err(failure(err)),
      };
      let pause = self.pause.saturating_mul(units);
      if deadline.is_some_and(|deadline| !fits(deadline, pause)) {
        return Err(last);
      }
      thread::sleep(pause);
      attempt += 1;
    }
  }

  /// One attempt; under a deadline nearer than the per-attempt timeout, the
  /// attempt's client times out at the deadline instead.
  fn send(
    &self,
    body: &[u8],
    auth: &Auth,
    deadline: Option<Instant>,
  ) -> Result<Response, toolu_http::Error> {
    let remaining = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
    if remaining.is_some_and(|left| left.is_zero()) {
      return Err(toolu_http::Error::Timeout);
    }
    let Some(http) = &self.http else {
      return Err(toolu_http::Error::Timeout);
    };
    let request = Request {
      method: Method::Post,
      url: &self.endpoint,
      auth,
      headers: &HEADERS,
      body: Some(body),
    };
    match remaining.filter(|left| *left < self.timeout) {
      Some(left) => match http_client(&self.env, left, self.root.clone()) {
        Ok(short) => short.send(&request),
        Err(Error::Transport(err)) => Err(err),
        Err(_) => Err(toolu_http::Error::Timeout),
      },
      None => http.send(&request),
    }
  }

  /// The pause before another attempt after a non-2xx `response`, or its error.
  fn retry_status(&self, response: &Response, attempt: u32) -> Result<u32, Error> {
    delay(response, attempt)
      .filter(|_| attempt < ATTEMPTS && retryable(response.status))
      .ok_or_else(|| self.http_error(response))
  }

  /// A non-2xx answer as an error, with the key redacted from its body.
  fn http_error(&self, response: &Response) -> Error {
    Error::Http {
      status: response.status,
      body: String::from_utf8_lossy(&response.body).replace(self.key.0.as_str(), "<redacted>"),
    }
  }
}

const HEADERS: [(&str, &str); 2] = [
  ("content-type", "application/json"),
  ("accept", "application/json"),
];

fn failure(err: toolu_http::Error) -> Error {
  match err {
    toolu_http::Error::Timeout => Error::Timeout,
    err @ (toolu_http::Error::InvalidConfig(_)
    | toolu_http::Error::HttpStatus(_)
    | toolu_http::Error::BodyTooLarge
    | toolu_http::Error::Encode(_)
    | toolu_http::Error::Decode(_)
    | toolu_http::Error::Transport(_)) => Error::Transport(err),
  }
}

/// Whether a `pause` still leaves time before `deadline` for another attempt.
fn fits(deadline: Instant, pause: Duration) -> bool {
  deadline
    .checked_duration_since(Instant::now())
    .is_some_and(|remaining| pause < remaining)
}

#[cfg(test)]
#[path = "tests/retry_test.rs"]
mod tests;
