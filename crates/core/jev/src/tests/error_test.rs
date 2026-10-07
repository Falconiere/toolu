use crate::Error;

#[test]
fn every_error_has_a_message() {
  let cases = [
    (Error::MissingKey, "TYPESAFE_API_KEY unset"),
    (
      Error::KeyLineBreak,
      "TYPESAFE_API_KEY must not contain line breaks",
    ),
    (
      Error::InvalidQuestion("score needs at least 2 levels".into()),
      "score needs at least 2 levels",
    ),
    (
      Error::Http {
        status: 401,
        body: r#"{"error":"bad key"}"#.into(),
      },
      r#"Jev answered HTTP 401: {"error":"bad key"}"#,
    ),
    (Error::Timeout, "the Jev request timed out"),
    (
      Error::Transport(toolu_http::Error::BodyTooLarge),
      "the Jev request failed: HTTP response body exceeds limit",
    ),
    (
      Error::InvalidResponse,
      "invalid response: expected a typed answer for every question",
    ),
  ];
  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
}
