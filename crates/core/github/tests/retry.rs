//! The one-shot policy keeps pr-babysit's gh retries (`gh.ts`); the scheduled
//! policy makes one attempt and never sleeps.

#[path = "helpers/api.rs"]
mod api;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use api::Api;
use toolu_github::{Client, Config, Error, RateLimit, Rest};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

fn env() -> Env {
  Env::from_pairs([("GH_TOKEN", "retry-token"), ("PB_GH_BACKOFF", "0 0 0")])
}

fn one_shot(api: &Api) -> Result<Client, Error> {
  api.client(Config::one_shot(&env())?, &env())
}

fn requests(api: &Api) -> usize {
  api.fixture.requests().map_or(0, |requests| requests.len())
}

#[test]
fn a_server_error_is_retried_until_it_succeeds() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence("/x", vec![Reply::new(502, ""), Reply::new(200, "{}")])
    .expect("route");
  let reply = one_shot(&api)
    .expect("client")
    .get("/x", None)
    .expect("reply");
  assert!(matches!(reply.data, Rest::Fresh(_)));
  assert_eq!(reply.attempts, 2);
}

#[test]
fn exhausted_attempts_return_the_last_status() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route("/x", Reply::new(502, r#"{"message":"Bad Gateway"}"#))
    .expect("route");
  assert_eq!(
    one_shot(&api).expect("client").get("/x", None).map(|_| ()),
    Err(Error::Status {
      status: 502,
      message: "Bad Gateway".into()
    })
  );
  assert_eq!(requests(&api), 3);
}

#[test]
fn a_not_found_and_a_plain_forbidden_are_permanent() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route("/missing", Reply::new(404, r#"{"message":"Not Found"}"#))
    .expect("route");
  api
    .fixture
    .route(
      "/forbidden",
      Reply::new(403, r#"{"message":"Resource not accessible"}"#),
    )
    .expect("route");
  let client = one_shot(&api).expect("client");
  assert_eq!(
    client.get("/missing", None).map(|_| ()),
    Err(Error::Status {
      status: 404,
      message: "Not Found".into()
    })
  );
  assert_eq!(
    client.get("/forbidden", None).map(|_| ()),
    Err(Error::Status {
      status: 403,
      message: "Resource not accessible".into()
    })
  );
  assert_eq!(requests(&api), 2);
}

#[test]
fn a_rate_limit_message_is_retried() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/x",
      vec![
        Reply::new(403, r#"{"message":"API rate limit exceeded for user"}"#),
        Reply::new(200, "{}"),
      ],
    )
    .expect("route");
  assert_eq!(
    one_shot(&api)
      .expect("client")
      .get("/x", None)
      .expect("reply")
      .attempts,
    2
  );
}

#[test]
fn a_short_retry_after_is_waited_for() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence(
      "/x",
      vec![
        Reply::new(403, "{}").header("Retry-After", "1"),
        Reply::new(200, "{}"),
      ],
    )
    .expect("route");
  let started = Instant::now();
  assert_eq!(
    one_shot(&api)
      .expect("client")
      .get("/x", None)
      .expect("reply")
      .attempts,
    2
  );
  assert!(started.elapsed() >= Duration::from_secs(1));
  assert_eq!(requests(&api), 2);
}

#[test]
fn a_long_wait_is_returned_at_once() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route(
      "/retry-after",
      Reply::new(403, "{}").header("Retry-After", "120"),
    )
    .expect("route");
  let reset = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("clock")
    .as_secs()
    + 3600;
  api
    .fixture
    .route(
      "/exhausted",
      Reply::new(403, "{}")
        .header("X-RateLimit-Remaining", "0")
        .header("X-RateLimit-Reset", &reset.to_string()),
    )
    .expect("route");
  let client = one_shot(&api).expect("client");
  assert_eq!(
    client.get("/retry-after", None).map(|_| ()),
    Err(Error::RateLimited {
      status: 403,
      retry_after: Some(Duration::from_secs(120)),
      rate: RateLimit::default(),
      secondary: false,
    })
  );
  let exhausted = client.get("/exhausted", None).map(|_| ());
  assert!(
    matches!(
      &exhausted,
      Err(Error::RateLimited { status: 403, retry_after: Some(wait), .. })
        if (3590..=3600).contains(&wait.as_secs())
    ),
    "{exhausted:?}"
  );
  assert_eq!(requests(&api), 2);
}

#[test]
fn an_invalid_variable_is_named() {
  for (name, value) in [
    ("PB_GH_ATTEMPTS", "0"),
    ("PB_GH_BACKOFF", "x"),
    ("PB_GH_TIMEOUT", "0"),
  ] {
    let result = Config::one_shot(&Env::from_pairs([(name, value)])).map(|_| ());
    assert!(
      matches!(&result, Err(Error::Config(message)) if message.starts_with(name)),
      "{name}={value}: {result:?}"
    );
  }
}

#[test]
fn a_dropped_connection_is_retried() {
  let api = Api::start().expect("api");
  api
    .fixture
    .sequence("/x", vec![Reply::dropped(), Reply::new(200, "{}")])
    .expect("route");
  assert_eq!(
    one_shot(&api)
      .expect("client")
      .get("/x", None)
      .expect("reply")
      .attempts,
    2
  );
  assert_eq!(api.fixture.connects().expect("connects").len(), 2);
  assert_eq!(requests(&api), 1);
}

#[test]
fn connections_dropped_every_time_end_in_a_transport_error() {
  let api = Api::start().expect("api");
  api.fixture.route("/x", Reply::dropped()).expect("route");
  assert!(matches!(
    one_shot(&api).expect("client").get("/x", None),
    Err(Error::Transport(toolu_http::Error::Transport(_)))
  ));
  assert_eq!(api.fixture.connects().expect("connects").len(), 3);
}

#[test]
fn a_reply_slower_than_the_timeout_times_out_every_attempt() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route(
      "/slow",
      Reply::new(200, "{}").delayed(Duration::from_millis(600)),
    )
    .expect("route");
  let mut config = Config::one_shot(&env()).expect("config");
  config.http.timeout = Duration::from_millis(150);
  let client = api.client(config, &env()).expect("client");
  assert_eq!(
    client.get("/slow", None).map(|_| ()),
    Err(Error::Transport(toolu_http::Error::Timeout))
  );
  assert_eq!(requests(&api), 3);
}

#[test]
fn a_rate_limit_that_persists_ends_with_the_attempts() {
  let api = Api::start().expect("api");
  api
    .fixture
    .route(
      "/x",
      Reply::new(403, r#"{"message":"API rate limit exceeded"}"#),
    )
    .expect("route");
  let env = env().with("PB_GH_ATTEMPTS", "2");
  let client = api
    .client(Config::one_shot(&env).expect("config"), &env)
    .expect("client");
  assert_eq!(
    client.get("/x", None).map(|_| ()),
    Err(Error::RateLimited {
      status: 403,
      retry_after: None,
      rate: RateLimit::default(),
      secondary: false,
    })
  );
  assert_eq!(requests(&api), 2);
}

#[test]
fn a_bad_path_or_etag_sends_nothing_and_never_waits() {
  let api = Api::start().expect("api");
  let client = one_shot(&api).expect("client");
  let started = Instant::now();
  for path in ["/repos/o/r/contents/a b", "/x<y>"] {
    assert_eq!(
      client.get(path, None).map(|_| ()),
      Err(Error::Config(format!(
        "{path:?} is not a URL path: percent-encode spaces, controls, non-ASCII and reserved \
         characters"
      )))
    );
  }
  assert_eq!(
    client.get("/x", Some("\"a\nb\"")).map(|_| ()),
    Err(Error::Config(r#""\"a\nb\"" is not an ETag"#.into()))
  );
  assert!(started.elapsed() < Duration::from_secs(1));
  assert_eq!(requests(&api), 0);
}
