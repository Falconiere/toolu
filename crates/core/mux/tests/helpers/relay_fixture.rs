use std::path::PathBuf;
use std::time::Duration;
use toolu_mux::hc::remote;
use toolu_mux::relay::{
  config::{self, Machine},
  store::{Store, Submission, Task},
};
use toolu_runtime::{
  env::Env,
  process::{Spec, run},
};

pub(crate) struct Fixture {
  _dir: tempfile::TempDir,
  pub(crate) env: Env,
  pub(crate) root: PathBuf,
  pub(crate) controller: PathBuf,
  worker: PathBuf,
}
impl Fixture {
  pub(crate) fn new() -> Result<Self, String> {
    let dir = tempfile::Builder::new()
      .prefix("relay-hc-")
      .tempdir_in("/tmp")
      .map_err(|e| e.to_string())?;
    let base = dir.path().to_str().ok_or("invalid temp path")?;
    let env = Env::process()
      .with("HC_SESSION", "local")
      .with("XDG_CONFIG_HOME", &format!("{base}/c"))
      .with("XDG_DATA_HOME", &format!("{base}/d"))
      .with("XDG_STATE_HOME", &format!("{base}/s"))
      .with("XDG_RUNTIME_DIR", &format!("{base}/r"));
    std::fs::create_dir_all(format!("{base}/r")).map_err(|e| e.to_string())?;
    let fixture = Self {
      root: dir.path().join("relay"),
      controller: dir.path().join("controller"),
      worker: dir.path().join("worker"),
      _dir: dir,
      env,
    };
    fixture.initialize()?;
    Ok(fixture)
  }
  fn initialize(&self) -> Result<(), String> {
    self.command(&[
      "hc",
      "machine",
      "add",
      "--name",
      "loop",
      "--via",
      "/bin/sh -c",
      "--session",
      "remotea",
      "--temp",
    ])?;
    std::fs::create_dir_all(&self.controller).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(self.worker.join("tasks")).map_err(|e| e.to_string())?;
    let repo = self.controller.to_str().ok_or("invalid path")?;
    self.git(repo, &["init", "-q"])?;
    self.git(repo, &["config", "user.name", "Test"])?;
    self.git(repo, &["config", "user.email", "test@example.invalid"])?;
    std::fs::write(self.controller.join("input"), b"base").map_err(|e| e.to_string())?;
    self.git(repo, &["add", "input"])?;
    self.git(repo, &["commit", "-qm", "base"])?;
    config::update(&self.root, |config| {
      config::register(
        config,
        Machine {
          name: "loop".into(),
          root: self.worker.to_str().ok_or("invalid path")?.into(),
          agents: vec!["codex".into()],
          capacity: 1,
          draining: false,
        },
      )
    })?;
    Ok(())
  }
  fn command(&self, args: &[&str]) -> Result<String, String> {
    let mut spec = Spec::new(args.iter().copied());
    spec.env = Some(self.env.clone());
    spec.timeout = Duration::from_secs(60);
    let out = run(&spec).map_err(|e| format!("{e:?}"))?;
    if out.exit_code != 0 || out.timed_out || out.truncated {
      return Err(format!("{args:?}: {}", out.stderr));
    }
    Ok(out.stdout.trim().to_owned())
  }
  pub(crate) fn git(&self, repo: &str, args: &[&str]) -> Result<String, String> {
    let mut command = vec!["git", "-C", repo];
    command.extend_from_slice(args);
    self.command(&command)
  }
  pub(crate) fn task(&self, key: &str) -> Result<(Task, String), String> {
    let repo = self.controller.to_str().ok_or("invalid path")?;
    let base = self.git(repo, &["rev-parse", "HEAD"])?;
    let bundle =
      remote::export_recover(&self.env, repo, key, &base).map_err(|e| format!("{e:?}"))?;
    let request = Submission {
      key: key.into(),
      origin: repo.into(),
      base,
      brief: "test".into(),
      agent: "codex".into(),
      argv: vec!["codex".into()],
      machine: None,
      group: None,
    };
    let store = Store::new(&self.root);
    store.submit(request, &bundle)?;
    let config = config::load(&self.root)?;
    Ok((
      store.reserve(key, &config, 1)?.ok_or("not reserved")?,
      repo.into(),
    ))
  }
  pub(crate) fn worktree(&self, repo: &str, attempt: &str) -> Result<String, String> {
    let worktree = self.worker.join("tasks").join(attempt);
    let path = worktree.to_str().ok_or("invalid path")?;
    self.git(repo, &["worktree", "add", "--detach", path, "HEAD"])?;
    Ok(path.into())
  }
  pub(crate) fn write_report(&self, attempt: &str, base: &str, commit: &str) -> Result<(), String> {
    let report =
      serde_json::json!({"attempt":attempt, "base":base, "status":"done", "commit":commit});
    std::fs::write(
      self
        .worker
        .join("tasks")
        .join(format!("{attempt}-report.json")),
      report.to_string(),
    )
    .map_err(|e| e.to_string())
  }
  pub(crate) fn terminal_name(attempt: &str) -> String {
    format!(
      "td-{}",
      attempt
        .chars()
        .filter(|c| *c != '-')
        .take(26)
        .collect::<String>()
    )
  }
}
impl Drop for Fixture {
  fn drop(&mut self) {
    for session in ["local", "remotea"] {
      let mut spec = Spec::new(["hc", "server", "stop"]);
      spec.env = Some(self.env.clone().with("HC_SESSION", session));
      let _ = run(&spec);
    }
  }
}
