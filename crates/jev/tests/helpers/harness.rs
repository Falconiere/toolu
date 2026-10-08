//! The loopback Jev endpoint a `toolu jev` verb runs against: a real TLS origin
//! behind a CONNECT proxy, the fixture CA on `Config`, and a 10 ms pause unit.

use std::time::Duration;

use serde_json::Value;
use toolu_http_test_support::{Fixture, Reply};
use toolu_jev_client::Config;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

/// The path `jev.ts` posts to.
pub(crate) const PATH: &str = "/v1/systemone";

/// A reply that passes the checks for one `noul` question under `q`.
pub(crate) const ANSWER: &str = r#"{"model":"jev-1.13.0","answers":{"q":{"type":"noul","noul":0.92}},"usage":{"input_tokens":3,"output_tokens":2}}"#;

/// The origin, its routes and what it observed.
pub(crate) struct Harness {
  /// The fixture.
  pub(crate) fixture: Fixture,
}

impl Harness {
  /// Start the origin and its proxy, answering `replies` in turn.
  pub(crate) fn start(replies: Vec<Reply>) -> Result<Harness, toolu_http_test_support::Error> {
    let fixture = Fixture::start()?;
    fixture.sequence(PATH, replies)?;
    Ok(Harness { fixture })
  }

  /// Run `toolu jev <args>` with `key`, `stdin` and `extra` environment on top.
  /// A usage error is the outcome the binary gives it: exit 64, clap's message.
  pub(crate) fn run(
    &self,
    args: &[&str],
    key: Option<&str>,
    stdin: Option<&str>,
    extra: &[(&str, &str)],
  ) -> Outcome {
    self
      .try_run(args, key, stdin, extra)
      .unwrap_or_else(|err| Outcome::failed(Exit::Usage, err.to_string()))
  }

  /// `run`, with clap's own error.
  pub(crate) fn try_run(
    &self,
    args: &[&str],
    key: Option<&str>,
    stdin: Option<&str>,
    extra: &[(&str, &str)],
  ) -> Result<Outcome, clap::Error> {
    let mut words = vec!["jev"];
    words.extend_from_slice(args);
    let matches = toolu_jev::command().try_get_matches_from(words)?;
    let mut env = Env::from_pairs([("HTTPS_PROXY", self.fixture.proxy_url())]);
    if let Some(key) = key {
      env = env.with("TYPESAFE_API_KEY", key);
    }
    for (name, value) in extra {
      env = env.with(name, value);
    }
    let config = Config {
      endpoint: self.fixture.url(PATH),
      pause: Duration::from_millis(10),
      test_root_ca_der: Some(self.fixture.root_ca_der().to_vec()),
    };
    Ok(toolu_jev::execute(
      &matches,
      &Ctx::default(),
      &env,
      config,
      stdin,
    ))
  }

  /// The bodies the endpoint received, as text.
  pub(crate) fn bodies(&self) -> Vec<String> {
    let requests = self.fixture.requests().unwrap_or_default();
    requests
      .into_iter()
      .map(|request| String::from_utf8_lossy(&request.body).into_owned())
      .collect()
  }

  /// The last body received, parsed.
  pub(crate) fn last(&self) -> Value {
    let parsed = self
      .bodies()
      .last()
      .and_then(|body| serde_json::from_str(body).ok());
    parsed.unwrap_or(Value::Null)
  }
}
