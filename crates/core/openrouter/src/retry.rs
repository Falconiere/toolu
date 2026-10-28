//! Three attempts share the caller's deadline, including waits and body reads.

use std::thread;
use std::time::{Duration, Instant};

use toolu_http::{Auth, Method, Request, Response};

use crate::{Client, Error, MAX_BODY_BYTES};

const ATTEMPTS: u32 = 3;

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
    let headers = [
      ("content-type", "application/json"),
      ("accept", "application/json"),
    ];
    for attempt in 1..=ATTEMPTS {
      let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(Error::Timeout)?;
      let http = toolu_http::Client::new(
        toolu_http::Config {
          timeout: remaining,
          max_body_bytes: MAX_BODY_BYTES,
          max_redirects: 0,
          test_root_ca_der: self.config.test_root_ca_der.clone(),
        },
        &self.env,
      )
      .map_err(|_err| Error::InvalidConfig)?;
      let outcome = http.send(&Request {
        method: Method::Post,
        url: &self.config.endpoint,
        auth: &auth,
        headers: &headers,
        body: Some(&body),
      });
      match outcome {
        Ok(response) if (200..300).contains(&response.status) => {
          return crate::response::parse(&response.body, &self.model);
        }
        Ok(response) => {
          let delay = status_delay(&response, attempt, self.config.pause)?;
          wait(deadline, delay, status(&response))?;
        }
        Err(err) => {
          let (delay, failure) = transport_delay(&err, attempt, self.config.pause)?;
          wait(deadline, delay, failure)?;
        }
      }
    }
    Err(Error::Timeout)
  }
}

fn status_delay(response: &Response, attempt: u32, pause: Duration) -> Result<Duration, Error> {
  if attempt == ATTEMPTS || !retryable(response.status) || no_provider(&response.body) {
    return Err(status(response));
  }
  Ok(retry_delay(response, attempt, pause))
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
    toolu_http::Error::InvalidConfig(_)
    | toolu_http::Error::HttpStatus(_)
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

fn status(response: &Response) -> Error {
  if no_provider(&response.body) {
    return Error::NoProvider;
  }
  match response.status {
    401 | 403 => Error::Authentication(response.status),
    429 => Error::RateLimited {
      retry_after_secs: retry_after(response),
    },
    code => Error::Upstream(code),
  }
}

fn no_provider(body: &[u8]) -> bool {
  let Ok(doc) = serde_json::from_slice::<serde_json::Value>(body) else {
    return false;
  };
  doc
    .get("error")
    .and_then(|error| error.get("message"))
    .and_then(serde_json::Value::as_str)
    .is_some_and(|message| {
      let message = message.to_ascii_lowercase();
      message.contains("no endpoints") || message.contains("no provider")
    })
}

fn retry_after(response: &Response) -> Option<u64> {
  response
    .header("retry-after")
    .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
    .and_then(|value| value.parse().ok())
}

fn retry_delay(response: &Response, attempt: u32, pause: Duration) -> Duration {
  let units = retry_after(response).unwrap_or(u64::from(attempt));
  pause.saturating_mul(u32::try_from(units).unwrap_or(u32::MAX))
}

fn fits(deadline: Instant, delay: Duration) -> bool {
  deadline
    .checked_duration_since(Instant::now())
    .is_some_and(|remaining| delay < remaining)
}

#[cfg(test)]
#[path = "tests/retry_test.rs"]
mod tests;
