//! A babysit tick that cannot read a GitHub token: `GH_TOKEN` is unset and
//! `gh auth token` fails. The real client returns its typed error, and the
//! engine's step turns it into an attention item naming both sources, with no
//! panic (#460).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickDecision, TickReport, TickRequest};
use toolu_epic_orchestrator::babysit::{Next, next};
use toolu_github::{Client, Config};
use toolu_runtime::env::Env;

/// A tick that needs a GitHub client before it can check anything.
struct GithubTick(Env);

impl BabysitTick for GithubTick {
  fn tick(&self, _request: &TickRequest) -> Result<TickReport, LinkError> {
    Client::new(Config::scheduled(), &self.0)
      .map(|_client| TickReport {
        decision: TickDecision::KeepGoing,
        result: Value::Null,
      })
      .map_err(|err| LinkError::Failed(err.to_string()))
  }
}

fn request() -> TickRequest {
  TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: None,
  }
}

fn step(env: Env) -> Next {
  next(&GithubTick(env), &request())
}

#[test]
fn a_failing_gh_becomes_an_attention_item_naming_both_sources() {
  let dir = tempfile::tempdir().expect("dir");
  fs::write(dir.path().join("hosts.yml"), "github.com: [broken\n").expect("hosts");
  let home = dir.path().to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("PATH", std::env::var("PATH").unwrap_or_default()),
    ("HOME", home.clone()),
    ("GH_CONFIG_DIR", home),
  ]);
  let Next::Attention(message) = step(env) else {
    panic!("expected an attention item");
  };
  assert!(
    message.starts_with(
      "babysit tick for Falconiere/toolu#460 failed: no GitHub token: GH_TOKEN is unset and \
       `gh auth token` failed: "
    ),
    "{message}"
  );
}

#[test]
fn a_missing_gh_becomes_an_attention_item_naming_both_sources() {
  let empty = tempfile::tempdir().expect("dir");
  let env = Env::from_pairs([("PATH", empty.path().to_string_lossy().into_owned())]);
  let Next::Attention(message) = step(env) else {
    panic!("expected an attention item");
  };
  assert!(
    message.contains("GH_TOKEN is unset and `gh auth token` failed: gh: "),
    "{message}"
  );
}
