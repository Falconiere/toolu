use std::io;

use crate::Error;
use crate::error::map_io;

#[test]
fn errors_have_actionable_messages() {
  let cases = [
    (
      Error::InvalidConfig("timeout".into()),
      "invalid HTTP configuration: timeout",
    ),
    (Error::HttpStatus(429), "HTTP status 429"),
    (Error::Timeout, "HTTP request timed out"),
    (Error::BodyTooLarge, "HTTP response body exceeds limit"),
    (
      Error::Encode("bad value".into()),
      "cannot encode JSON request: bad value",
    ),
    (
      Error::Decode("bad JSON".into()),
      "cannot decode JSON response: bad JSON",
    ),
    (
      Error::Transport("offline".into()),
      "HTTP transport error: offline",
    ),
  ];
  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
}

#[test]
fn a_timed_out_read_is_a_timeout_and_any_other_read_failure_transport() {
  assert_eq!(
    map_io(&io::Error::new(io::ErrorKind::TimedOut, "slow")),
    Error::Timeout
  );
  assert_eq!(
    map_io(&io::Error::new(io::ErrorKind::ConnectionReset, "reset")),
    Error::Transport("reset".into())
  );
}
