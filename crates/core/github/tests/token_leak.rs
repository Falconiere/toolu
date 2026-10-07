//! The token never leaves the client (AC-3). This test binary has no libtest
//! harness: run plainly it is the parent, and it runs itself as a child with a
//! sentinel token. The child drives every GitHub path with it, prints every
//! result and error with `{}` and `{:?}`, appends each call's cost or error to a
//! journal as JSON lines, and ends in a panic whose message carries the client
//! and the last error. The parent then searches the child's stdout, stderr and
//! journal for the sentinel.
//!
//! `main` is not test code to clippy, so it unwraps nothing and returns errors.

#[path = "helpers/api.rs"]
mod api;
#[path = "helpers/gh.rs"]
mod gh;

use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use api::Api;
use gh::Gh;
use serde_json::json;
use toolu_github::{Client, Config, Error, Retry};
use toolu_http_test_support::Reply;
use toolu_runtime::env::Env;

/// Set in the child to the sentinel token.
const CHILD: &str = "TOOLU_GITHUB_LEAK_CHILD";
/// Set in the child to the journal path.
const JOURNAL: &str = "TOOLU_GITHUB_LEAK_JOURNAL";
/// The scenarios the child runs; the parent checks it got through all of them.
const SCENARIOS: usize = 10;

type Res<T> = Result<T, String>;

fn main() -> ExitCode {
  if let Ok(token) = std::env::var(CHILD) {
    return child(&token);
  }
  match parent() {
    Ok(()) => ExitCode::SUCCESS,
    Err(err) => {
      std::io::stderr()
        .write_all(format!("token_leak: {err}\n").as_bytes())
        .ok();
      ExitCode::FAILURE
    }
  }
}

/// Run the child and search everything it left for the sentinel.
fn parent() -> Res<()> {
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|err| err.to_string())?
    .as_nanos();
  let sentinel = format!("toolu-sentinel-{}-{nanos}", std::process::id());
  let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
  let journal = dir.path().join("journal.jsonl");
  let exe = std::env::current_exe().map_err(|err| err.to_string())?;
  let output = Command::new(exe)
    .env(CHILD, &sentinel)
    .env(JOURNAL, &journal)
    .output()
    .map_err(|err| err.to_string())?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  let journal = fs::read_to_string(&journal).map_err(|err| err.to_string())?;
  let ran = format!("scenarios: {SCENARIOS}");
  if !stdout.contains(&ran) || !stderr.contains("deliberate panic") {
    return Err(format!(
      "the child did not run to its panic:\n{stdout}\n{stderr}"
    ));
  }
  if !stdout.contains("control: the fixture saw the token") {
    return Err("the positive control failed: no request carried the token".into());
  }
  for (name, text) in [
    ("stdout", &*stdout),
    ("stderr", &*stderr),
    ("journal", &journal),
  ] {
    if text.contains(&sentinel) {
      return Err(format!("the token leaked into the child's {name}"));
    }
  }
  std::io::stdout()
    .write_all(b"token_leak: ok\n")
    .map_err(|err| err.to_string())
}

/// What the child printed and journaled.
struct Log {
  journal: File,
  count: usize,
}

impl Log {
  /// Print `result` with `{:?}`, its error with `{}`, and journal its cost or error.
  fn record<T: Debug>(
    &mut self,
    name: &str,
    result: &Result<T, Error>,
    cost: Option<String>,
  ) -> Res<()> {
    let (text, line) = match result {
      Ok(_) => (
        format!("{name}: {result:?}\n"),
        cost.unwrap_or_else(|| "{}".into()),
      ),
      Err(err) => (
        format!("{name}: {result:?}\n{name}: {err}\n"),
        json!({ "scenario": name, "error": err.to_string() }).to_string(),
      ),
    };
    std::io::stdout()
      .write_all(text.as_bytes())
      .map_err(|err| err.to_string())?;
    writeln!(self.journal, "{line}").map_err(|err| err.to_string())?;
    self.count += 1;
    Ok(())
  }
}

/// The one-shot policy without backoff.
fn quick() -> Retry {
  Retry {
    attempts: 3,
    backoff: vec![Duration::ZERO; 3],
    max_wait: Duration::from_secs(60),
  }
}

fn config(retry: Retry) -> Config {
  Config {
    retry,
    ..Config::scheduled()
  }
}

/// Every GitHub path with the sentinel token, then the deliberate panic.
fn child(token: &str) -> ExitCode {
  match scenarios(token) {
    Ok((log, client, last)) => {
      assert!(
        log.count == 0,
        "deliberate panic after {} scenarios: client {client:?}, last error {last:?} / {last}",
        log.count
      );
      ExitCode::SUCCESS
    }
    Err(err) => {
      std::io::stderr().write_all(err.as_bytes()).ok();
      ExitCode::FAILURE
    }
  }
}

fn scenarios(token: &str) -> Res<(Log, Client, Error)> {
  let journal = std::env::var(JOURNAL).map_err(|err| err.to_string())?;
  let mut log = Log {
    journal: File::create(Path::new(&journal)).map_err(|err| err.to_string())?,
    count: 0,
  };
  let api = Api::start().map_err(|err| err.to_string())?;
  routes(&api, token)?;
  let client = calls(&api, token, &mut log)?;
  rotation(&api, token, &mut log)?;
  control(&api, token)?;
  let last = failures(&api, token, &mut log)?;
  std::io::stdout()
    .write_all(format!("scenarios: {}\n", log.count).as_bytes())
    .map_err(|err| err.to_string())?;
  Ok((log, client, last))
}

/// Replies that echo the token where GitHub could.
fn routes(api: &Api, token: &str) -> Res<()> {
  let graphql = json!({ "errors": [{ "message": format!("token {token}") }] }).to_string();
  let routes = [
    (
      "/ok",
      Reply::new(200, "{}").header("X-RateLimit-Remaining", "10"),
    ),
    ("/same", Reply::new(304, "")),
    (
      "/unauthorized",
      Reply::new(401, format!(r#"{{"message":"{token}"}}"#)),
    ),
    (
      "/fail",
      Reply::new(502, format!(r#"{{"message":"bad token {token}"}}"#)),
    ),
    (
      "/limited",
      Reply::new(403, "{}").header("Retry-After", "120"),
    ),
    ("/graphql", Reply::new(200, graphql)),
  ];
  for (path, reply) in routes {
    api
      .fixture
      .route(path, reply)
      .map_err(|err| err.to_string())?;
  }
  Ok(())
}

/// The calls a client with the token makes: success, 304, 401, 5xx, rate
/// limit and GraphQL errors.
fn calls(api: &Api, token: &str, log: &mut Log) -> Res<Client> {
  let env = Env::from_pairs([("GH_TOKEN", token)]);
  let client = api
    .client(config(quick()), &env)
    .map_err(|err| err.to_string())?;
  let reply = client.get("/ok", None);
  let cost = reply
    .as_ref()
    .ok()
    .and_then(|reply| serde_json::to_string(&reply.cost).ok());
  log.record("success", &reply, cost)?;
  log.record("not-modified", &client.get("/same", Some("\"v1\"")), None)?;
  log.record("unauthorized", &client.get("/unauthorized", None), None)?;
  log.record("server-error", &client.get("/fail", None), None)?;
  log.record("rate-limit", &client.get("/limited", None), None)?;
  let graphql = client.graphql("{ viewer { login } }", &json!({}));
  log.record("graphql", &graphql.map(|_| ()), None)?;
  Ok(client)
}

/// A refused connection, a failing `gh` and a malformed token: the last error.
fn failures(api: &Api, token: &str, log: &mut Log) -> Res<Error> {
  let env = Env::from_pairs([("GH_TOKEN", token)]);
  let unreachable = Config {
    api_url: "https://127.0.0.1:1".into(),
    ..config(quick())
  };
  let refused = Client::new(unreachable, &env).and_then(|client| client.get("/x", None));
  log.record("refused", &refused, None)?;
  let gh = Gh::new().map_err(|err| err.to_string())?;
  gh.broken(token).map_err(|err| err.to_string())?;
  let failed = api.client(config(quick()), &gh.env()).map(|_| ());
  log.record("failed-gh", &failed, None)?;
  let malformed = Env::from_pairs([("GH_TOKEN", format!("{token} x"))]);
  let result = api.client(config(quick()), &malformed).map(|_| ());
  log.record("malformed", &result, None)?;
  result
    .err()
    .ok_or_else(|| "a malformed token was accepted".to_owned())
}

/// A token rotated after a `401`, then a server error that echoes the old one.
fn rotation(api: &Api, token: &str, log: &mut Log) -> Res<()> {
  let (old, new) = (format!("{token}-old"), format!("{token}-new"));
  let gh = Gh::new().map_err(|err| err.to_string())?;
  gh.token(&old).map_err(|err| err.to_string())?;
  let client = api
    .client(config(quick()), &gh.env())
    .map_err(|err| err.to_string())?;
  gh.token(&new).map_err(|err| err.to_string())?;
  let echo = format!(r#"{{"message":"{old} was replaced by {new}"}}"#);
  api
    .fixture
    .sequence(
      "/rotate",
      vec![Reply::new(401, "{}"), Reply::new(502, echo)],
    )
    .map_err(|err| err.to_string())?;
  let result = client.get("/rotate", None);
  if let Err(Error::Status { message, .. }) = &result
    && message != "<redacted> was replaced by <redacted>"
  {
    return Err("the rotated tokens were not redacted".into());
  }
  log.record("rotation", &result, None)
}

/// The positive control: the fixture saw the token on the wire.
fn control(api: &Api, token: &str) -> Res<()> {
  let bearer = format!("Bearer {token}");
  let seen = api
    .fixture
    .requests()
    .map_err(|err| err.to_string())?
    .iter()
    .any(|request| request.headers.get("authorization") == Some(&bearer));
  if !seen {
    return Err("no request carried the token".into());
  }
  std::io::stdout()
    .write_all(b"control: the fixture saw the token\n")
    .map_err(|err| err.to_string())
}
