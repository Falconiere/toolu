use std::path::Path;
use std::process::Command;

use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use toolu_http_test_support::Fixture;
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
  std::fs::write(root.join(".codex/toolu.config.json"), "{}").unwrap();
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

/// A PATH of the system tools plus links to the git and ast-grep the process
/// PATH resolves, under `state`. Their own directories stay out: a host CLI
/// beside them (codex sits with ast-grep in Homebrew's bin) would be selected
/// to review instead of the fixture's host executable.
fn tools_path(state: &Path) -> String {
  let bin = state.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let process = Env::process();
  let dirs: Vec<&str> = process.get("PATH").unwrap_or("").split(':').collect();
  for tool in ["git", "ast-grep"] {
    let link = bin.join(tool);
    // A relative PATH entry would make a link that dangles outside the cwd.
    let found = dirs
      .iter()
      .map(|dir| Path::new(dir).join(tool))
      .find(|path| path.is_absolute() && path.is_file());
    // A second environment under the same dir finds the links already made.
    if let Some(found) = found
      && std::fs::symlink_metadata(&link).is_err()
    {
      std::os::unix::fs::symlink(found, link).unwrap();
    }
  }
  format!("{}:/usr/bin:/bin", bin.display())
}

pub(super) fn env(root: &Path, state: &Path, fixture: &Fixture) -> Env {
  Env::process()
    .with("TOOLU_PROJECT_DIR", &root.to_string_lossy())
    .with("STATE_DIR", &state.to_string_lossy())
    .with("TYPESAFE_API_KEY", "private-sentinel-669")
    .with("HTTPS_PROXY", &fixture.proxy_url())
    .with("NO_PROXY", "")
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with(
      "PATH",
      &format!("{}:{}", state.display(), tools_path(state)),
    )
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
    skip_judge: true,
    ..Options::default()
  }
}

pub(super) fn host(state: &Path, response: &str) {
  let path = state.join("codex");
  std::fs::write(&path, format!("#!/bin/sh\nprintf '%s' '{response}'\n")).unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

pub(super) fn clean_case() -> (tempfile::TempDir, tempfile::TempDir, Fixture) {
  let repo = repo();
  let state = tempfile::tempdir().unwrap();
  let fixture = Fixture::start().unwrap();
  host(state.path(), &json!({"findings": []}).to_string());
  (repo, state, fixture)
}

pub(super) fn finding_case(approved: bool) -> (tempfile::TempDir, tempfile::TempDir, Fixture) {
  let repo = repo();
  let state = tempfile::tempdir().unwrap();
  let fixture = finding_fixture(state.path(), approved);
  (repo, state, fixture)
}

fn finding_fixture(state: &Path, approved: bool) -> Fixture {
  let fixture = Fixture::start().unwrap();
  let finding = json!({"path":"src.txt","line":1,"end_line":1,"side":"new",
    "severity":"high","category":"correctness","claim":"the new value breaks callers",
    "quote":"new","suggestion":null,"prior_id":null});
  let response = if approved {
    json!({"verdict":"approved","findings":[finding]})
  } else {
    json!({"findings":[finding]})
  };
  host(state, &response.to_string());
  fixture
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

pub(super) fn review_with_finding(root: &Path, state: &Path) -> Env {
  let fixture = finding_fixture(state, false);
  let env = env(root, state, &fixture);
  let report = invoke(
    &env,
    &Options {
      skip_judge: true,
      ..options(&fixture)
    },
  );
  assert!(
    report
      .findings
      .iter()
      .any(|item| item.path == "src.txt" && item.severity.blocking())
  );
  env
}
