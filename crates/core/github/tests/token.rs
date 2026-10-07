//! The token sources against a real `gh` and the loopback API: `hosts.yml`,
//! rotation on a `401`, and the typed errors when no token can be read.

#[path = "helpers/api.rs"]
mod api;
#[path = "helpers/gh.rs"]
mod gh;

use std::sync::Barrier;
use std::thread;
use std::time::Duration;

use api::Api;
use gh::Gh;
use toolu_github::{Config, Error, Rest, Source, TokenError};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

fn authorizations(api: &Api) -> Vec<String> {
  api
    .fixture
    .requests()
    .unwrap_or_default()
    .iter()
    .map(|request| {
      request
        .headers
        .get("authorization")
        .cloned()
        .unwrap_or_default()
    })
    .collect()
}

#[test]
fn the_gh_token_is_sent_and_reread_after_a_401() {
  let api = Api::start().expect("api");
  let gh = Gh::new().expect("gh");
  gh.token("token-before").expect("hosts");
  let client = api.client(Config::scheduled(), &gh.env()).expect("client");
  gh.token("token-after").expect("rotate");
  api
    .fixture
    .sequence("/user", vec![Reply::new(401, "{}"), Reply::new(200, "{}")])
    .expect("route");
  let reply = client.get("/user", None).expect("reply");
  assert!(matches!(reply.data, Rest::Fresh(_)));
  assert_eq!(reply.attempts, 1);
  assert_eq!(
    authorizations(&api),
    ["Bearer token-before", "Bearer token-after"]
  );
}

#[test]
fn a_401_with_an_unchanged_token_is_unauthorized_after_one_request() {
  let api = Api::start().expect("api");
  let env = Env::from_pairs([("GH_TOKEN", "fixed-token")]);
  let client = api.client(Config::scheduled(), &env).expect("client");
  api
    .fixture
    .route("/user", Reply::new(401, "{}"))
    .expect("route");
  assert_eq!(client.get("/user", None), Err(Error::Unauthorized));
  assert_eq!(authorizations(&api), ["Bearer fixed-token"]);
}

#[test]
fn a_failing_gh_names_both_sources() {
  let api = Api::start().expect("api");
  let gh = Gh::new().expect("gh");
  gh.broken("not yaml").expect("hosts");
  let Err(Error::Token(TokenError::Unavailable { gh: reason })) =
    api.client(Config::scheduled(), &gh.env())
  else {
    panic!("expected Unavailable");
  };
  assert!(reason.contains("invalid"), "{reason}");
  let message = TokenError::Unavailable { gh: reason }.to_string();
  assert!(message.starts_with("no GitHub token: GH_TOKEN is unset and `gh auth token` failed: "));
}

#[test]
fn a_missing_gh_names_both_sources() {
  let api = Api::start().expect("api");
  let empty = tempfile::tempdir().expect("dir");
  let env = Env::from_pairs([("PATH", empty.path().to_string_lossy().into_owned())]);
  let Err(Error::Token(TokenError::Unavailable { gh })) = api.client(Config::scheduled(), &env)
  else {
    panic!("expected Unavailable");
  };
  assert_eq!(gh, "gh: No such file or directory (os error 2)");
}

#[test]
fn a_malformed_gh_token_sends_nothing() {
  let api = Api::start().expect("api");
  for value in ["a b", "a\nb"] {
    let env = Env::from_pairs([("GH_TOKEN", value)]);
    assert_eq!(
      api.client(Config::scheduled(), &env).map(|_| ()),
      Err(Error::Token(TokenError::Malformed(Source::Env)))
    );
  }
  assert_eq!(api.fixture.requests().expect("requests").len(), 0);
}

#[test]
fn a_rotated_token_that_is_refused_too_is_unauthorized_after_two_requests() {
  let api = Api::start().expect("api");
  let gh = Gh::new().expect("gh");
  gh.token("token-before").expect("hosts");
  let client = api.client(Config::scheduled(), &gh.env()).expect("client");
  gh.token("token-after").expect("rotate");
  api
    .fixture
    .route("/user", Reply::new(401, "{}"))
    .expect("route");
  assert_eq!(client.get("/user", None), Err(Error::Unauthorized));
  assert_eq!(
    authorizations(&api),
    ["Bearer token-before", "Bearer token-after"]
  );
}

#[test]
fn a_token_that_cannot_be_reread_is_unauthorized_after_one_request() {
  let api = Api::start().expect("api");
  let gh = Gh::new().expect("gh");
  gh.token("token-before").expect("hosts");
  let client = api.client(Config::scheduled(), &gh.env()).expect("client");
  gh.broken("gone").expect("break");
  api
    .fixture
    .route("/user", Reply::new(401, "{}"))
    .expect("route");
  assert_eq!(client.get("/user", None), Err(Error::Unauthorized));
  assert_eq!(authorizations(&api), ["Bearer token-before"]);
}

#[test]
fn two_threads_refused_together_both_retry_with_the_rotated_token() {
  let api = Api::start().expect("api");
  let gh = Gh::new().expect("gh");
  gh.token("token-before").expect("hosts");
  let client = api.client(Config::scheduled(), &gh.env()).expect("client");
  gh.token("token-after").expect("rotate");
  let refused = Reply::new(401, "{}").delayed(Duration::from_millis(300));
  api
    .fixture
    .sequence(
      "/user",
      vec![refused.clone(), refused, Reply::new(200, "{}")],
    )
    .expect("route");
  let start = Barrier::new(2);
  let results: Vec<bool> = thread::scope(|scope| {
    let calls: Vec<_> = (0..2)
      .map(|_| {
        scope.spawn(|| {
          start.wait();
          client.get("/user", None).is_ok()
        })
      })
      .collect();
    calls
      .into_iter()
      .map(|call| call.join().unwrap_or(false))
      .collect()
  });
  assert_eq!(results, [true, true]);
  let mut seen = authorizations(&api);
  seen.sort();
  assert_eq!(
    seen,
    [
      "Bearer token-after",
      "Bearer token-after",
      "Bearer token-before",
      "Bearer token-before"
    ]
  );
}
