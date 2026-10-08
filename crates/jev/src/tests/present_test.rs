use toolu_jev_client::{Error, Questions, State};
use toolu_protocol::exit::Exit;

use super::{call_error, error, reply};
use crate::call::CallError;

fn checked(body: &str) -> toolu_jev_client::Reply {
  // A reply is only built by the client, so run one through a loopback call.
  let fixture = toolu_http_test_support::Fixture::start().unwrap();
  fixture
    .route(
      "/v1/systemone",
      toolu_http_test_support::Reply::new(200, body),
    )
    .unwrap();
  let env = toolu_runtime::env::Env::from_pairs([
    ("TYPESAFE_API_KEY", "k".to_owned()),
    ("HTTPS_PROXY", fixture.proxy_url()),
  ]);
  let config = toolu_jev_client::Config {
    endpoint: fixture.url("/v1/systemone"),
    pause: std::time::Duration::from_millis(10),
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
  };
  let jev = toolu_jev_client::Jev::from_env(&env, config).unwrap();
  let questions = Questions::parse(
    r#"{"b":{"type":"noul","instructions":"x"},"a":{"type":"noul","instructions":"y"}}"#,
  )
  .unwrap();
  jev.ask(&State::text("s"), "m", &questions).unwrap()
}

const BODY: &str = r#"{"model":"jev-1","answers":{"a":{"type":"noul","noul":0.5},"b":{"type":"noul","noul":0.25}},"usage":{"input_tokens":1,"output_tokens":2}}"#;

#[test]
fn answers_print_compact_in_question_order_without_a_newline() {
  let outcome = reply(&checked(BODY), false);
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(r#"{"b":{"type":"noul","noul":0.25},"a":{"type":"noul","noul":0.5}}"#)
  );
  assert_eq!(outcome.stderr, None);
}

#[test]
fn raw_prints_the_body_pretty_with_two_spaces() {
  let outcome = reply(&checked(BODY), true);
  let text = outcome.stdout.unwrap();
  assert!(text.starts_with("{\n  \"model\": \"jev-1\",\n  \"answers\": {\n    \"a\": {"));
  assert!(text.ends_with('}'));
}

#[test]
fn errors_map_to_their_exits_and_messages() {
  let line = |err: Error| {
    let outcome = error(&err);
    (outcome.exit, outcome.stdout, outcome.stderr)
  };
  assert_eq!(
    line(Error::MissingKey),
    (
      Exit::Failure,
      None,
      Some("jev: TYPESAFE_API_KEY unset".to_owned())
    )
  );
  assert_eq!(
    line(Error::Timeout),
    (
      Exit::TempFail,
      None,
      Some("jev: the Jev request timed out".to_owned())
    )
  );
  assert_eq!(
    line(Error::Http {
      status: 401,
      body: "{\"error\":\"bad key\"}\n".into()
    }),
    (
      Exit::Failure,
      None,
      Some("{\"error\":\"bad key\"}".to_owned())
    )
  );
  assert_eq!(
    line(Error::Http {
      status: 502,
      body: String::new()
    })
    .2
    .as_deref(),
    Some("jev: Jev answered HTTP 502: ")
  );
  assert_eq!(
    line(Error::InvalidQuestion(
      "choice needs at least 2 options".into()
    ))
    .2
    .as_deref(),
    Some("jev: choice needs at least 2 options")
  );
}

#[test]
fn call_errors_are_failures_with_the_jev_prefix() {
  let outcome = call_error(&CallError::Input("cannot read stdin".into()));
  assert_eq!(outcome.exit, Exit::Failure);
  assert_eq!(outcome.stderr.as_deref(), Some("jev: cannot read stdin"));
  let outcome = call_error(&CallError::Client(Error::InvalidResponse));
  assert!(outcome.stderr.unwrap().starts_with("jev: invalid response"));
}
