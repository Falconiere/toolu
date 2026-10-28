//! A real committed diff in a temp Git repository, shaped, gathered and
//! pre-passed by the production stages, and one loopback HTTPS origin standing in
//! for openrouter.ai: `/api/v1/chat/completions` for the review model and
//! `/api/v1/systemone` for Jev.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;
use toolu_code_review::budget::Effort;
use toolu_code_review::memory::Memory;
use toolu_code_review::prepass::{self, PrepassResult};
use toolu_code_review::review::{self, Input, Options, Reviewed, RunError};
use toolu_code_review::rules::{self, Bundle};
use toolu_code_review::settings::Settings;
use toolu_code_review::shape::{Shaped, shape};
use toolu_http_test_support::Fixture;
use toolu_runtime::env::Env;

pub(crate) const REVIEW: &str = "/api/v1/chat/completions";
pub(crate) const JEV: &str = "/api/v1/systemone";
pub(crate) const KEY: &str = "private-sentinel-666";

/// One fixture repository, its pipeline inputs and the loopback origin.
pub(crate) struct Rig {
  pub(crate) fixture: Fixture,
  pub(crate) repo: TempDir,
  pub(crate) shaped: Shaped,
  pub(crate) rules: Bundle,
  pub(crate) prepass: PrepassResult,
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
  let status = Command::new("git")
    .current_dir(root)
    .args(["-c", "user.name=T", "-c", "user.email=t@example.com"])
    .args(args)
    .env("GIT_CONFIG_NOSYSTEM", "1")
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .env("XDG_CONFIG_HOME", "/dev/null")
    .status()
    .map_err(|err| err.to_string())?;
  status
    .success()
    .then_some(())
    .ok_or_else(|| format!("git {args:?} failed"))
}

fn write(root: &Path, files: &[(&str, String)]) -> Result<(), String> {
  for (path, text) in files {
    let path = root.join(path);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(path, text).map_err(|err| err.to_string())?;
  }
  Ok(())
}

impl Rig {
  /// `base` committed on `main`, `change` committed on `feature`.
  pub(crate) fn new(base: &[(&str, String)], change: &[(&str, String)]) -> Result<Rig, String> {
    let repo = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = repo.path();
    git(root, &["init", "-q", "-b", "main"])?;
    write(root, base)?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-qm", "base"])?;
    git(root, &["switch", "-qc", "feature"])?;
    write(root, change)?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-qm", "change"])?;
    let fixture = Fixture::start().map_err(|err| err.to_string())?;
    let env = git_env();
    let shaped = shape(&env, root, Some("main")).map_err(|err| format!("{err:?}"))?;
    let rules = rules::gather(&env, root, &shaped.merge_base, &shaped.gate_files)
      .map_err(|err| format!("{err:?}"))?;
    let prepass = prepass::run(&env, root, &shaped);
    Ok(Rig {
      fixture,
      repo,
      shaped,
      rules,
      prepass,
    })
  }

  /// The process environment a run sees: the key, the proxy, an isolated Git.
  pub(crate) fn env(&self) -> Env {
    git_env()
      .with("OPENROUTER_API_KEY", KEY)
      .with("OPENROUTER_MODEL", "")
      .with("JEV_TIMEOUT", "5")
      .with("HTTPS_PROXY", &self.fixture.proxy_url())
      .with("NO_PROXY", "")
  }

  /// Both clients aimed at the loopback origin, with 10 ms pauses.
  pub(crate) fn options(&self) -> Options<'static> {
    let root = Some(self.fixture.root_ca_der().to_vec());
    Options {
      openrouter: toolu_openrouter::Config {
        endpoint: self.fixture.url(REVIEW),
        pause: Duration::from_millis(10),
        test_root_ca_der: root.clone(),
      },
      jev: Some(toolu_jev_client::Config {
        endpoint: self.fixture.url(JEV),
        pause: Duration::from_millis(10),
        test_root_ca_der: root,
      }),
      ..Options::default()
    }
  }

  /// Run the review stage over this rig's inputs.
  pub(crate) fn run(
    &self,
    memory: &Memory,
    flag: Option<Effort>,
    options: &Options<'_>,
  ) -> Result<Reviewed, RunError> {
    let settings = Settings {
      enabled: true,
      ..Settings::default()
    };
    self.run_with(memory, &settings, flag, options)
  }

  /// `run` with explicit settings.
  pub(crate) fn run_with(
    &self,
    memory: &Memory,
    settings: &Settings,
    flag: Option<Effort>,
    options: &Options<'_>,
  ) -> Result<Reviewed, RunError> {
    let input = Input {
      root: self.repo.path(),
      shaped: &self.shaped,
      rules: &self.rules,
      prepass: &self.prepass,
      memory,
      settings,
      flag,
    };
    review::run(&self.env(), &input, options)
  }

  /// The bodies `path` received, in order.
  pub(crate) fn bodies(&self, path: &str) -> Vec<String> {
    self
      .fixture
      .requests()
      .unwrap_or_default()
      .into_iter()
      .filter(|request| request.path == path)
      .map(|request| String::from_utf8_lossy(&request.body).into_owned())
      .collect()
  }
}

fn git_env() -> Env {
  let mut env = Env::from_pairs([
    ("GIT_CONFIG_NOSYSTEM", "1".to_owned()),
    ("GIT_CONFIG_GLOBAL", "/dev/null".to_owned()),
    ("XDG_CONFIG_HOME", "/dev/null".to_owned()),
  ]);
  let process = Env::process();
  for name in ["PATH", "HOME", "TMPDIR"] {
    if let Some(value) = process.get(name) {
      env = env.with(name, value);
    }
  }
  env
}

/// A completed structured call whose visible content is `content`.
pub(crate) fn completion(content: &Value) -> String {
  json!({
    "model": "deepseek/deepseek-v4-pro-0813",
    "provider": "Parasail",
    "choices": [{"message": {"content": content.to_string()}, "finish_reason": "stop"}],
    "usage": {"prompt_tokens": 100, "completion_tokens": 40, "cost": 0.01}
  })
  .to_string()
}

/// A review answer with `findings`.
pub(crate) fn findings(findings: &[Value]) -> String {
  completion(&json!({ "findings": findings }))
}

/// One model finding on `path`.
pub(crate) fn finding(path: &str, severity: &str, claim: &str, prior_id: Option<&str>) -> Value {
  json!({"path": path, "line": 1, "end_line": 1, "side": "new", "severity": severity,
         "category": "correctness", "claim": claim, "quote": "x", "suggestion": null,
         "prior_id": prior_id})
}
