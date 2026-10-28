//! Real loopback HTTPS checks for structured `OpenRouter` calls.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_openrouter::{Client, Config, Error, Message, StructuredRequest};
use toolu_runtime::env::Env;

const PATH: &str = "/api/v1/chat/completions";
const MESSAGES: [Message<'static>; 1] = [Message {
  role: "user",
  content: "Review this change",
}];

fn client(fixture: &Fixture, extra: &[(&str, &str)]) -> Result<Client, Error> {
  let mut env = Env::from_pairs([
    ("OPENROUTER_API_KEY", "private-sentinel".to_owned()),
    ("HTTPS_PROXY", fixture.proxy_url()),
  ]);
  for (key, value) in extra {
    env = env.with(key, value);
  }
  Client::from_env(
    &env,
    Config {
      endpoint: fixture.url(PATH),
      pause: Duration::from_millis(10),
      test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    },
  )
}

fn request(schema: &Value) -> StructuredRequest<'_> {
  StructuredRequest {
    messages: &MESSAGES,
    schema_name: "code_review",
    schema,
    reasoning_effort: "high",
    max_tokens: 65_536,
  }
}

fn response(content: &str) -> String {
  json!({
    "model":"deepseek/deepseek-v4-pro-0813",
    "provider":"Parasail",
    "choices":[{"message":{"content":content},"finish_reason":"stop"}],
    "usage":{"prompt_tokens":12,"completion_tokens":20,"completion_tokens_details":{"reasoning_tokens":5},"cost":0.0125}
  }).to_string()
}

#[test]
fn structured_request_keeps_schema_reasoning_and_routing() {
  let fixture = Fixture::start().expect("TLS fixture");
  fixture
    .route(PATH, Reply::new(200, response("{\"findings\":[]}")))
    .expect("route");
  let schema = json!({"type":"object","properties":{"findings":{"type":"array"}},"required":["findings"],"additionalProperties":false});
  let result = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2))
    .expect("completion");
  assert_eq!(result.content, "{\"findings\":[]}");
  assert_eq!(result.provider.as_deref(), Some("Parasail"));
  assert_eq!(result.usage.cost_usd, Some(0.0125));
  let seen = fixture.requests().expect("requests");
  let body: Value = serde_json::from_slice(&seen[0].body).expect("body");
  assert_eq!(body["model"], "deepseek/deepseek-v4-pro-0813");
  assert_eq!(body["response_format"]["json_schema"]["strict"], true);
  assert_eq!(body["response_format"]["json_schema"]["schema"], schema);
  assert_eq!(body["reasoning"], json!({"effort":"high","exclude":true}));
  assert_eq!(
    body["provider"],
    json!({"require_parameters":true,"zdr":true,"quantizations":["fp8","bf16","fp16"]})
  );
  assert_eq!(body["max_tokens"], 65_536);
  assert_eq!(
    seen[0].headers.get("authorization").map(String::as_str),
    Some("Bearer private-sentinel")
  );
}

#[test]
fn missing_key_is_unconfigured_and_never_sends() {
  let fixture = Fixture::start().expect("TLS fixture");
  let config = Config {
    endpoint: fixture.url(PATH),
    pause: Duration::from_millis(10),
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
  };
  let env = Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())]);
  assert!(matches!(
    Client::from_env(&env, config),
    Err(Error::Unconfigured)
  ));
  assert_eq!(fixture.requests().expect("requests").len(), 0);
  assert_eq!(fixture.connects().expect("connects").len(), 0);
}

#[test]
fn rate_limit_retries_but_auth_does_not() {
  let fixture = Fixture::start().expect("TLS fixture");
  fixture
    .sequence(
      PATH,
      vec![
        Reply::new(429, "busy").header("Retry-After", "0"),
        Reply::new(200, response("{\"findings\":[]}")),
      ],
    )
    .expect("route");
  let schema = json!({"type":"object"});
  assert!(
    client(&fixture, &[])
      .expect("client")
      .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2))
      .is_ok()
  );
  assert_eq!(fixture.requests().expect("requests").len(), 2);
  fixture
    .route(PATH, Reply::new(401, "private-sentinel"))
    .expect("route");
  let err = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2))
    .expect_err("auth");
  assert!(matches!(err, Error::Authentication(401)));
  assert!(!format!("{err:?} {err}").contains("private-sentinel"));
  assert_eq!(fixture.requests().expect("requests").len(), 3);
}

#[test]
fn missing_cost_stays_unknown_and_override_changes_model() {
  let fixture = Fixture::start().expect("TLS fixture");
  let body = json!({"choices":[{"message":{"content":"{}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}).to_string();
  fixture.route(PATH, Reply::new(200, body)).expect("route");
  let schema = json!({"type":"object"});
  let result = client(&fixture, &[("OPENROUTER_MODEL", "another/model")])
    .expect("client")
    .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2))
    .expect("completion");
  assert_eq!(result.usage.cost_usd, None);
  let seen = fixture.requests().expect("requests");
  let body: Value = serde_json::from_slice(&seen[0].body).expect("body");
  assert_eq!(body["model"], "another/model");
  assert_eq!(body["provider"]["zdr"], true);
}

#[test]
fn reasoning_only_length_and_empty_are_typed_errors() {
  let fixture = Fixture::start().expect("TLS fixture");
  let schema = json!({"type":"object"});
  let length = json!({"choices":[{"message":{"content":""},"finish_reason":"length"}],"usage":{"prompt_tokens":12,"completion_tokens":100,"completion_tokens_details":{"reasoning_tokens":100},"cost":0.5}}).to_string();
  fixture.route(PATH, Reply::new(200, length)).expect("route");
  let err = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2))
    .expect_err("truncated");
  assert!(matches!(err, Error::Truncated { .. }));
  assert_eq!(err.usage().and_then(|(usage, _)| usage.cost_usd), Some(0.5));
  assert!(format!("{err}").contains("length"));
  fixture
    .route(PATH, Reply::new(200, response("")))
    .expect("route");
  assert!(matches!(
    client(&fixture, &[])
      .expect("client")
      .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2)),
    Err(Error::Empty { .. })
  ));
}

#[test]
fn an_oversized_body_is_typed_and_never_retried() {
  let fixture = Fixture::start().expect("TLS fixture");
  let oversized = "x".repeat(toolu_openrouter::MAX_BODY_BYTES + 1);
  fixture
    .route(PATH, Reply::new(200, oversized))
    .expect("route");
  let schema = json!({"type":"object"});
  let result = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2));
  assert!(matches!(result, Err(Error::BodyTooLarge)));
  assert_eq!(fixture.requests().expect("requests").len(), 1);
}

#[test]
fn server_failure_and_dropped_connection_retry_inside_one_deadline() {
  let fixture = Fixture::start().expect("TLS fixture");
  fixture
    .sequence(
      PATH,
      vec![
        Reply::new(503, "down"),
        Reply::dropped(),
        Reply::new(200, response("{}")),
      ],
    )
    .expect("route");
  let schema = json!({"type":"object"});
  let started = Instant::now();
  assert!(
    client(&fixture, &[])
      .expect("client")
      .complete_structured(&request(&schema), started + Duration::from_secs(2))
      .is_ok()
  );
  assert!(started.elapsed() < Duration::from_secs(2));
  assert_eq!(fixture.requests().expect("requests").len(), 2);
  assert!(fixture.connects().expect("connects").len() >= 3);
}

#[test]
fn delayed_response_and_retry_after_cannot_extend_the_deadline() {
  let fixture = Fixture::start().expect("TLS fixture");
  let schema = json!({"type":"object"});
  fixture
    .route(PATH, Reply::new(200, response("{}")))
    .expect("route");
  let started = Instant::now();
  let result = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), started);
  assert!(matches!(result, Err(Error::Timeout)));
  assert_eq!(fixture.requests().expect("requests").len(), 0);
  fixture
    .route(PATH, Reply::new(429, "busy").header("Retry-After", "500"))
    .expect("route");
  let started = Instant::now();
  let result = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), started + Duration::from_secs(2));
  assert!(matches!(
    result,
    Err(Error::RateLimited {
      retry_after_secs: Some(500)
    })
  ));
  assert!(started.elapsed() < Duration::from_secs(2));
  assert_eq!(fixture.requests().expect("requests").len(), 1);
  fixture
    .route(
      PATH,
      Reply::new(200, response("{}")).delayed(Duration::from_millis(100)),
    )
    .expect("route");
  let started = Instant::now();
  let result = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), started + Duration::from_millis(30));
  assert!(matches!(result, Err(Error::Timeout)));
  assert!(started.elapsed() < Duration::from_millis(150));
}

#[test]
fn explicit_no_provider_and_embedded_error_fail_closed() {
  let fixture = Fixture::start().expect("TLS fixture");
  let schema = json!({"type":"object"});
  fixture
    .route(
      PATH,
      Reply::new(
        404,
        json!({"error":{"message":"No endpoints found"}}).to_string(),
      ),
    )
    .expect("route");
  assert!(matches!(
    client(&fixture, &[])
      .expect("client")
      .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(1)),
    Err(Error::NoProvider)
  ));
  fixture.route(PATH, Reply::new(200, json!({"choices":[{"message":{"content":"partial"},"finish_reason":"error","error":{"code":502,"message":"provider failed"}}]}).to_string())).expect("route");
  assert!(matches!(
    client(&fixture, &[])
      .expect("client")
      .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(1)),
    Err(Error::Upstream(502))
  ));
}

#[test]
fn embedded_transient_error_retries_with_the_same_routing() {
  let fixture = Fixture::start().expect("TLS fixture");
  let schema = json!({"type":"object"});
  let embedded = json!({"choices":[{"message":{"content":"partial"},"finish_reason":"error","error":{"code":429,"message":"busy"}}]}).to_string();
  fixture
    .sequence(
      PATH,
      vec![Reply::new(200, embedded), Reply::new(200, response("{}"))],
    )
    .expect("route");
  let completion = client(&fixture, &[])
    .expect("client")
    .complete_structured(&request(&schema), Instant::now() + Duration::from_secs(2))
    .expect("retried");
  assert_eq!(completion.content, "{}");
  let seen = fixture.requests().expect("requests");
  assert_eq!(seen.len(), 2);
  for request in seen {
    let body: Value = serde_json::from_slice(&request.body).expect("body");
    assert_eq!(body["provider"]["zdr"], true);
    assert_eq!(body["provider"]["require_parameters"], true);
  }
}
