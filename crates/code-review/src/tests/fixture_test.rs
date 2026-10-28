use std::path::Path;
use std::process::Command;

use serde_json::json;
use toolu_http_test_support::{Fixture, Reply};
use toolu_protocol::host::Host;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::run;
use crate::cli;
use crate::review::Options;

fn git(root: &Path, args: &[&str]) {
  let out = Command::new("git")
    .current_dir(root)
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .output()
    .unwrap();
  assert!(
    out.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&out.stderr)
  );
}

pub(super) fn repo() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  git(root, &["init", "-q", "-b", "main"]);
  git(root, &["config", "user.name", "Test"]);
  git(root, &["config", "user.email", "test@example.com"]);
  std::fs::create_dir_all(root.join(".codex")).unwrap();
  std::fs::write(root.join(".gitignore"), ".codex/tmp/\n").unwrap();
  std::fs::write(
    root.join(".codex/toolu.config.json"),
    r#"{"codeReview":{"enabled":true}}"#,
  )
  .unwrap();
  std::fs::write(root.join("src.txt"), "old\n").unwrap();
  git(root, &["add", "-A"]);
  git(root, &["commit", "-qm", "base"]);
  git(root, &["switch", "-qc", "feature"]);
  std::fs::write(root.join("src.txt"), "new\n").unwrap();
  std::fs::write(root.join("café.rs"), "pub fn value() -> i32 { 2 }\n").unwrap();
  git(root, &["add", "-A"]);
  git(root, &["commit", "-qm", "change"]);
  dir
}

pub(super) fn env(root: &Path, state: &Path, fixture: &Fixture) -> Env {
  let mut env = Env::process()
    .with("TOOLU_PROJECT_DIR", &root.to_string_lossy())
    .with("STATE_DIR", &state.to_string_lossy())
    .with("OPENROUTER_API_KEY", "private-sentinel-669")
    .with("HTTPS_PROXY", &fixture.proxy_url())
    .with("NO_PROXY", "")
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null");
  if let Some(path) = Env::process().get("PATH") {
    env = env.with("PATH", path);
  }
  env
}

pub(super) fn options(fixture: &Fixture) -> Options<'static> {
  let ca = Some(fixture.root_ca_der().to_vec());
  Options {
    openrouter: toolu_openrouter::Config {
      endpoint: fixture.url("/api/v1/chat/completions"),
      pause: std::time::Duration::from_millis(10),
      test_root_ca_der: ca.clone(),
    },
    jev: Some(toolu_jev_client::Config {
      endpoint: fixture.url("/api/v1/systemone"),
      pause: std::time::Duration::from_millis(10),
      test_root_ca_der: ca,
    }),
    ..Options::default()
  }
}

pub(super) fn body(findings: &serde_json::Value) -> String {
  body_doc(&json!({"findings":findings}))
}

pub(super) fn body_doc(doc: &serde_json::Value) -> String {
  json!({"model":"deepseek/deepseek-v4-pro-0813","provider":"Parasail",
    "provider_quantization":"fp8",
    "choices":[{"message":{"content":doc.to_string()},"finish_reason":"stop"}],
    "usage":{"prompt_tokens":100,"completion_tokens":20,"cost":0.01}})
  .to_string()
}

pub(super) fn clean_case() -> (tempfile::TempDir, tempfile::TempDir, Fixture) {
  let repo = repo();
  let state = tempfile::tempdir().unwrap();
  let fixture = Fixture::start().unwrap();
  fixture
    .route(
      "/api/v1/chat/completions",
      Reply::new(200, body(&json!([]))),
    )
    .unwrap();
  (repo, state, fixture)
}

pub(super) fn finding_case(approved: bool) -> (tempfile::TempDir, tempfile::TempDir, Fixture) {
  let repo = repo();
  let state = tempfile::tempdir().unwrap();
  let fixture = Fixture::start().unwrap();
  let finding = json!({"path":"src.txt","line":1,"end_line":1,"side":"new",
    "severity":"high","category":"correctness","claim":"the new value breaks callers",
    "quote":"new","suggestion":null,"prior_id":null});
  let response = if approved {
    body_doc(&json!({"verdict":"approved","findings":[finding]}))
  } else {
    body(&json!([finding]))
  };
  fixture
    .route("/api/v1/chat/completions", Reply::new(200, response))
    .unwrap();
  (repo, state, fixture)
}

pub(super) fn saved_state(state: &Path) -> serde_json::Value {
  serde_json::from_str(&std::fs::read_to_string(state.join("feature.json")).unwrap()).unwrap()
}

pub(super) fn invoke(env: &Env, options: &Options<'_>) -> crate::contract::Report {
  let matches = cli::command()
    .try_get_matches_from(["code-review", "run", "--base", "main", "--effort", "medium"])
    .unwrap();
  let ctx = Ctx {
    host: Some(Host::Codex),
    json: true,
    ..Ctx::default()
  };
  run(
    env,
    &ctx,
    matches.subcommand_matches("run").unwrap(),
    options,
  )
  .unwrap()
}
