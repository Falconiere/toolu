//! `jev.ts`'s attempts: three, with `retry-after` or exponential pauses for
//! 408, 429 and 5xx answers and for transport failures.

use std::thread;

use toolu_http::{Auth, Method, Request, Response};

use crate::{Error, Jev};

/// Attempts per call.
const ATTEMPTS: u32 = 3;

/// The longest wait, in pause units, before a status is returned instead.
const MAX_UNITS: u64 = 60;

/// Whether `status` is worth another attempt.
pub(crate) fn retryable(status: u16) -> bool {
  status == 408 || status == 429 || status >= 500
}

/// The pause units before the attempt after `attempt` (1-based); `None` when
/// the status is returned instead.
pub(crate) fn delay(response: &Response, attempt: u32) -> Option<u64> {
  let base = 1_u64 << attempt.saturating_sub(1).min(16);
  let digits = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
  let value = match (
    response.header("retry-after"),
    response.header("retry-after-ms"),
  ) {
    (Some(seconds), _) if digits(seconds) => {
      if seconds.len() > 2 {
        return None;
      }
      seconds.parse::<u64>().ok()?
    }
    (_, Some(millis)) if digits(millis) => {
      if millis.len() > 8 {
        return None;
      }
      millis.parse::<u64>().ok()?.div_ceil(1000)
    }
    _ => base,
  };
  (value <= MAX_UNITS).then_some(value.max(base))
}

impl Jev {
  /// POST `body` with up to three attempts: the 2xx body, or the last failure.
  pub(crate) fn post(&self, body: &[u8]) -> Result<String, Error> {
    let auth = Auth::Bearer(self.key.0.clone());
    let headers = [
      ("content-type", "application/json"),
      ("accept", "application/json"),
    ];
    let mut attempt = 1;
    loop {
      let outcome = match &self.http {
        Some(http) => http.send(&Request {
          method: Method::Post,
          url: &self.endpoint,
          auth: &auth,
          headers: &headers,
          body: Some(body),
        }),
        None => Err(toolu_http::Error::Timeout),
      };
      let units = match outcome {
        Ok(response) if (200..300).contains(&response.status) => {
          return Ok(String::from_utf8_lossy(&response.body).into_owned());
        }
        Ok(response) => self.retry_status(&response, attempt)?,
        Err(toolu_http::Error::Timeout | toolu_http::Error::Transport(_)) if attempt < ATTEMPTS => {
          1_u64 << (attempt - 1)
        }
        Err(toolu_http::Error::Timeout) => return Err(Error::Timeout),
        Err(err) => return Err(Error::Transport(err)),
      };
      self.pause(units);
      attempt += 1;
    }
  }

  /// The pause before another attempt after a non-2xx `response`, or its error.
  fn retry_status(&self, response: &Response, attempt: u32) -> Result<u64, Error> {
    delay(response, attempt)
      .filter(|_| attempt < ATTEMPTS && retryable(response.status))
      .ok_or_else(|| Error::Http {
        status: response.status,
        body: String::from_utf8_lossy(&response.body).replace(self.key.0.as_str(), "<redacted>"),
      })
  }

  fn pause(&self, units: u64) {
    let units = u32::try_from(units).unwrap_or(u32::MAX);
    thread::sleep(self.pause.saturating_mul(units));
  }
}

#[cfg(test)]
#[path = "tests/retry_test.rs"]
mod tests;
