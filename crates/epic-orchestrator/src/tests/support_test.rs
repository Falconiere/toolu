//! Shared test fixtures: clients that trust the loopback HTTPS fixture.

use toolu_http_test_support::Fixture;
use toolu_runtime::env::Env;

use crate::retry::Policy;

/// Fast retries for fixture tests: two attempts, 1 ms backoff, 2 s cap.
pub(crate) const FAST: Policy = Policy {
  attempts: 2,
  base_ms: 1,
  max_sleep_ms: 2000,
};

/// The environment that routes HTTPS through the fixture's proxy.
pub(crate) fn proxy_env(fixture: &Fixture) -> Env {
  Env::from_pairs([("HTTPS_PROXY", fixture.proxy_url())])
}

/// A `toolu-http` client that trusts the fixture.
pub(crate) fn http_client(fixture: &Fixture) -> toolu_http::Client {
  let config = toolu_http::Config {
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
    ..toolu_http::Config::default()
  };
  toolu_http::Client::new(config, &proxy_env(fixture)).expect("http client")
}

/// A GitHub client against the fixture, with token `token`.
pub(crate) fn github_client(fixture: &Fixture, token: &str) -> toolu_github::Client {
  let mut config = toolu_github::Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  let env = proxy_env(fixture).with("GH_TOKEN", token);
  toolu_github::Client::new(config, &env).expect("github client")
}

/// The tracker's GitHub wrapper over the fixture, with [`FAST`] retries.
pub(crate) fn gh(fixture: &Fixture) -> crate::tracker::Gh {
  crate::tracker::Gh::with_client(github_client(fixture, "fixture-token"), FAST)
}

/// Route `path` to a `200` with the JSON `body`.
pub(crate) fn ok(fixture: &Fixture, path: &str, body: &serde_json::Value) {
  fixture
    .route(
      path,
      toolu_http_test_support::Reply::new(200, body.to_string()),
    )
    .expect("route");
}

/// Jira settings pointing at the fixture: a PAT, API `version`, one attempt.
pub(crate) fn jira_env(fixture: &Fixture, version: &str) -> Env {
  proxy_env(fixture)
    .with("JIRA_BASE_URL", &fixture.url(""))
    .with("JIRA_PAT", "pat-token")
    .with("JIRA_API_VERSION", version)
}

/// The committed epic #248 graph (`fixtures/epic/epic248-graph.json`).
pub(crate) fn graph_248() -> crate::graph::Graph {
  serde_json::from_str(include_str!("../../../../fixtures/epic/epic248-graph.json")).expect("graph")
}

/// Run git in `cwd` with a fixed identity; panics on failure (tests only).
pub(crate) fn git(cwd: &std::path::Path, args: &[&str]) -> String {
  let output = std::process::Command::new("git")
    .args(args)
    .current_dir(cwd)
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@t")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@t")
    .output()
    .expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository at `dir` with one commit and `origin` set to `url`.
pub(crate) fn repo_with_origin(dir: &std::path::Path, url: &str) {
  std::fs::create_dir_all(dir).expect("dir");
  git(dir, &["init", "-q", "-b", "main"]);
  std::fs::write(dir.join("a.txt"), "one\n").expect("file");
  git(dir, &["add", "a.txt"]);
  git(dir, &["commit", "-qm", "init"]);
  git(dir, &["remote", "add", "origin", url]);
}

/// A Jev client aimed at the fixture's `/v1/systemone`.
pub(crate) fn jev(fixture: &Fixture) -> toolu_jev_client::Jev {
  let config = toolu_jev_client::Config {
    endpoint: fixture.url("/v1/systemone"),
    pause: std::time::Duration::from_millis(1),
    test_root_ca_der: Some(fixture.root_ca_der().to_vec()),
  };
  let env = proxy_env(fixture).with("TYPESAFE_API_KEY", "fixture-key");
  toolu_jev_client::Jev::from_env(&env, config).expect("jev")
}

/// Effect clients that reach the fixture: GitHub under `policy`, GitHub
/// epics through it with no close polls, Jira (API 3) on the fixture;
/// Linear stays live.
pub(crate) fn reach(fixture: &Fixture, policy: Policy) -> crate::reach::Reach {
  let (url, ca, proxy) = (
    fixture.url(""),
    fixture.root_ca_der().to_vec(),
    fixture.proxy_url(),
  );
  let gh: std::sync::Arc<crate::reach::MakeGh> = std::sync::Arc::new(move |_env: &Env| {
    let mut config = toolu_github::Config::scheduled();
    config.api_url.clone_from(&url);
    config.http.test_root_ca_der = Some(ca.clone());
    let env = Env::from_pairs([
      ("HTTPS_PROXY", proxy.as_str()),
      ("GH_TOKEN", "fixture-token"),
    ]);
    toolu_github::Client::new(config, &env)
      .map(|client| crate::tracker::Gh::with_client(client, policy))
      .map_err(|err| err.to_string())
  });
  let tracker_gh = std::sync::Arc::clone(&gh);
  let (jira, jira_http) = (jira_env(fixture, "3"), http_client(fixture));
  let polls = crate::tracker::github::ClosePolls {
    count: 0,
    wait: std::time::Duration::ZERO,
  };
  crate::reach::Reach {
    gh,
    tracker: std::sync::Arc::new(move |kind, reference: &str, repo: &str, env: &Env| {
      if kind == crate::tracker::TrackerKind::Jira {
        let http = crate::tracker::rest::Http::with_client(jira_http.clone(), policy, "epic jira");
        let client = crate::tracker::jira_client::JiraClient::with_http(&jira, http)?;
        let tracker = crate::tracker::jira::JiraTracker::with(reference, repo, client)?;
        return Ok(Box::new(tracker) as Box<dyn crate::tracker::Tracker + Sync>);
      }
      if kind != crate::tracker::TrackerKind::Github {
        return crate::tracker::make_tracker(kind, reference, repo, env);
      }
      let tracker =
        crate::tracker::github::GitHubTracker::with(reference, tracker_gh(env)?, polls)?;
      Ok(Box::new(tracker) as Box<dyn crate::tracker::Tracker + Sync>)
    }),
  }
}

/// The `toolu epic <verb>` matches of `line`, as the dispatcher hands them on.
pub(crate) fn matches(line: &[&str]) -> clap::ArgMatches {
  let all = crate::verbs::command()
    .try_get_matches_from(line)
    .expect("line");
  all
    .subcommand()
    .map(|(_, sub)| sub.clone())
    .expect("subcommand")
}

/// A bare `origin` on `main` and its clone at `checkout`, with `files`
/// committed and pushed.
pub(crate) fn origin_with_clone(
  origin: &std::path::Path,
  checkout: &std::path::Path,
  files: &[(&str, &str)],
) {
  std::fs::create_dir_all(origin).expect("origin");
  git(origin, &["init", "-q", "--bare", "-b", "main"]);
  let parent = checkout.parent().expect("parent");
  std::fs::create_dir_all(parent).expect("parent dir");
  let (from, to) = (origin.display().to_string(), checkout.display().to_string());
  git(parent, &["clone", "-q", &from, &to]);
  for (name, text) in files {
    std::fs::write(checkout.join(name), text).expect("file");
  }
  git(checkout, &["add", "."]);
  git(checkout, &["commit", "-qm", "init"]);
  git(checkout, &["push", "-q", "origin", "main"]);
}

/// Issue `a` of the source fixture reports `ready` with PR 412, then the
/// engine runs what that armed.
pub(crate) fn report_ready(engine: &mut crate::server::Engine) {
  // The source fixture's graph names no repository; the merge queue needs one.
  if let Some(issue) = engine.world.issues.get_mut("a") {
    "o/r".clone_into(&mut issue.repo);
  }
  let ready = crate::model::Report {
    key: "a".into(),
    epic: "one".into(),
    state_dir: engine.world.issues["a"].state_dir.clone(),
    phase: "ready".into(),
    pr: Some(412),
    note: String::new(),
  };
  engine.report(&ready).expect("report");
  engine.pump().expect("pump");
}
