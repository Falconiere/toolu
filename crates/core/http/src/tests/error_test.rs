use std::io;

use crate::Error;
use crate::error::{map_io, map_ureq};

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
      Error::NotSent("host not found".into()),
      "HTTP request was not sent: host not found",
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

#[test]
fn dns_and_connect_failures_are_known_before_send_but_resets_are_not() {
  for error in [
    ureq::Error::HostNotFound,
    ureq::Error::ConnectionFailed,
    ureq::Error::ConnectProxyFailed("proxy".into()),
    ureq::Error::Io(io::Error::new(io::ErrorKind::ConnectionRefused, "refused")),
  ] {
    let mapped = map_ureq(&error);
    assert!(matches!(mapped, Error::NotSent(_)), "{mapped:?}");
  }
  assert_eq!(
    map_ureq(&ureq::Error::Io(io::Error::new(
      io::ErrorKind::ConnectionReset,
      "reset"
    ))),
    Error::Transport("reset".into())
  );
}
