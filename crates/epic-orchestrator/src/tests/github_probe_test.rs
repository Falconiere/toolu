use serde_json::json;
use toolu_github::{Client, Config};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use super::{Change, probe};
use crate::watch::{Kind, Watch};

fn client(fixture: &Fixture) -> Client {
  let mut config = Config::scheduled();
  config.api_url = fixture.url("");
  config.http.test_root_ca_der = Some(fixture.root_ca_der().to_vec());
  let env = Env::from_pairs([
    ("GH_TOKEN", "detector-token"),
    ("HTTPS_PROXY", &fixture.proxy_url()),
  ]);
  Client::new(config, &env).expect("client")
}

fn route(fixture: &Fixture, path: &str, body: &str) {
  fixture
    .sequence(
      path,
      vec![
        Reply::new(200, body).header("ETag", &format!("\"{}\"", path.len())),
        Reply::new(304, "").header("X-RateLimit-Used", "6"),
      ],
    )
    .expect("route");
}

#[test]
fn github_detectors_pr_probe_uses_etags_and_keeps_rest_primary_flat() {
  let fixture = Fixture::start().expect("fixture");
  let root = "/repos/Falconiere/toolu";
  route(
    &fixture,
    &format!("{root}/pulls/501"),
    &json!({"state":"open", "merged":false, "head":{"sha":"abc"}, "base":{"ref":"main"}})
      .to_string(),
  );
  route(
    &fixture,
    &format!("{root}/commits/abc/check-runs?per_page=100"),
    r#"{"check_runs":[]}"#,
  );
  route(&fixture, &format!("{root}/commits/abc/status"), "{}");
  for path in [
    format!("{root}/issues/501/comments?per_page=100"),
    format!("{root}/pulls/501/comments?per_page=100"),
    format!("{root}/pulls/501/reviews?per_page=100"),
  ] {
    route(&fixture, &path, "[]");
  }
  let mut watch = Watch::new(
    Kind::Pr {
      key: "toolu-447".into(),
      repo: "Falconiere/toolu".into(),
      number: 501,
    },
    0,
  );
  let api = client(&fixture);
  let first = probe(&api, &mut watch).expect("first");
  assert_eq!(first.rest_points, 6);
  assert_eq!(first.fresh, 6);
  assert!(
    first
      .changes
      .iter()
      .any(|change| matches!(change, Change::PrChanged { .. }))
  );
  let second = probe(&api, &mut watch).expect("second");
  assert_eq!((second.rest_points, second.not_modified), (0, 6));
  assert_eq!(second.changes, Vec::new());
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 12);
  assert!(
    requests[6..]
      .iter()
      .all(|request| request.headers.contains_key("if-none-match"))
  );
}

#[test]
fn github_detectors_epic_pagination_survives_conditional_replies() {
  let fixture = Fixture::start().expect("fixture");
  let first = "/repos/Falconiere/toolu/issues/402/sub_issues?per_page=100";
  let second = "/repos/Falconiere/toolu/issues/402/sub_issues?per_page=100&page=2";
  let link = format!("<{}>; rel=\"next\"", fixture.url(second));
  fixture
    .sequence(
      first,
      vec![
        Reply::new(200, r#"[{"number":447}]"#)
          .header("ETag", "\"page-one\"")
          .header("Link", &link),
        Reply::new(304, ""),
      ],
    )
    .expect("first route");
  fixture
    .sequence(
      second,
      vec![
        Reply::new(200, r#"[{"number":460}]"#).header("ETag", "\"page-two\""),
        Reply::new(304, ""),
      ],
    )
    .expect("second route");
  let mut watch = Watch::new(
    Kind::Epic {
      key: "falconiere-toolu-402".into(),
      repo: "Falconiere/toolu".into(),
      number: 402,
    },
    0,
  );
  let api = client(&fixture);
  assert_eq!(probe(&api, &mut watch).expect("first").fresh, 2);
  let second_check = probe(&api, &mut watch).expect("second");
  assert_eq!(
    (second_check.rest_points, second_check.not_modified),
    (0, 2)
  );
  let requests = fixture.requests().expect("requests");
  assert_eq!(requests.len(), 4);
  assert_eq!(
    requests[3].headers.get("if-none-match").map(String::as_str),
    Some("\"page-two\"")
  );
}

#[test]
fn github_detectors_base_ref_changes_are_inputs_and_invalid_repo_sends_nothing() {
  let fixture = Fixture::start().expect("fixture");
  let path = "/repos/Falconiere/toolu/git/ref/heads/main";
  route(&fixture, path, r#"{"object":{"sha":"main-sha"}}"#);
  let api = client(&fixture);
  let mut watch = Watch::new(
    Kind::Base {
      repo: "Falconiere/toolu".into(),
      branch: "main".into(),
    },
    0,
  );
  assert!(matches!(
    probe(&api, &mut watch).expect("first").changes.as_slice(),
    [Change::BaseMoved { repo, branch }] if repo == "Falconiere/toolu" && branch == "main"
  ));
  assert_eq!(probe(&api, &mut watch).expect("second").not_modified, 1);
  let mut invalid = Watch::new(
    Kind::Base {
      repo: "Falconiere/toolu/foreign".into(),
      branch: "main".into(),
    },
    0,
  );
  assert!(probe(&api, &mut invalid).is_err());
  assert_eq!(fixture.requests().expect("requests").len(), 2);
}

#[test]
fn github_detectors_head_change_drops_old_commit_etags() {
  let fixture = Fixture::start().expect("fixture");
  route_head_change(&fixture);
  let mut watch = Watch::new(
    Kind::Pr {
      key: "toolu-447".into(),
      repo: "Falconiere/toolu".into(),
      number: 501,
    },
    0,
  );
  let api = client(&fixture);
  probe(&api, &mut watch).expect("first");
  probe(&api, &mut watch).expect("second");
  assert_eq!(watch.head_sha, "def");
  assert!(
    watch
      .etags
      .keys()
      .all(|path| !path.contains("/commits/abc/"))
  );
  assert!(
    watch
      .etags
      .keys()
      .any(|path| path.contains("/commits/def/"))
  );
}

fn route_head_change(fixture: &Fixture) {
  let root = "/repos/Falconiere/toolu";
  fixture
    .sequence(
      &format!("{root}/pulls/501"),
      vec![
        Reply::new(
          200,
          r#"{"state":"open","merged":false,"head":{"sha":"abc"},"base":{"ref":"main"}}"#,
        )
        .header("ETag", "\"pr-abc\""),
        Reply::new(
          200,
          r#"{"state":"open","merged":false,"head":{"sha":"def"},"base":{"ref":"main"}}"#,
        )
        .header("ETag", "\"pr-def\""),
      ],
    )
    .expect("PR route");
  for head in ["abc", "def"] {
    route(
      fixture,
      &format!("{root}/commits/{head}/check-runs?per_page=100"),
      r#"{"check_runs":[]}"#,
    );
    route(fixture, &format!("{root}/commits/{head}/status"), "{}");
  }
  for path in [
    format!("{root}/issues/501/comments?per_page=100"),
    format!("{root}/pulls/501/comments?per_page=100"),
    format!("{root}/pulls/501/reviews?per_page=100"),
  ] {
    route(fixture, &path, "[]");
  }
}
