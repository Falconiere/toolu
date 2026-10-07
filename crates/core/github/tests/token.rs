//! The token sources against a real `gh` and the loopback API: `hosts.yml`,
//! rotation on a `401`, and the typed errors when no token can be read.

#[path = "helpers/api.rs"]
mod api;
#[path = "helpers/gh.rs"]
mod gh;

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
  assert!(gh.starts_with("gh: "), "{gh}");
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
