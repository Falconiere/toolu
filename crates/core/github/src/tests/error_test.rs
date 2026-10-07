use std::time::Duration;

use crate::Error;
use crate::token::TokenError;

#[test]
fn every_error_has_a_message() {
  let cases = [
    (
      Error::from(TokenError::Unavailable { gh: "x".into() }),
      "no GitHub token: GH_TOKEN is unset and `gh auth token` failed: x",
    ),
    (
      Error::Config("PB_GH_ATTEMPTS must be a positive integer".into()),
      "GitHub client configuration: PB_GH_ATTEMPTS must be a positive integer",
    ),
    (
      Error::Unauthorized,
      "GitHub rejected the token (401), also after re-reading it",
    ),
    (
      Error::RateLimited {
        status: 403,
        retry_after: Some(Duration::from_secs(120)),
      },
      "GitHub rate limit (HTTP 403): retry in 120s",
    ),
    (
      Error::RateLimited {
        status: 429,
        retry_after: None,
      },
      "GitHub rate limit (HTTP 429)",
    ),
  ];
  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
}

#[test]
fn status_graphql_transport_and_decode_errors_have_messages() {
  let cases = [
    (
      Error::Status {
        status: 404,
        message: String::new(),
      },
      "GitHub answered HTTP 404",
    ),
    (
      Error::Status {
        status: 422,
        message: "Validation Failed".into(),
      },
      "GitHub answered HTTP 422: Validation Failed",
    ),
    (
      Error::GraphQl(vec!["a".into(), "b".into()]),
      "GitHub GraphQL errors: a; b",
    ),
    (
      Error::Transport(toolu_http::Error::Timeout),
      "GitHub request failed: HTTP request timed out",
    ),
    (
      Error::Decode("eof".into()),
      "cannot decode the GitHub reply: eof",
    ),
  ];
  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
}
