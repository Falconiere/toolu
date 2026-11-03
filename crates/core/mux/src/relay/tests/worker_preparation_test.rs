//! Real HC and Git checks for worker preparation and recovery.

use super::{build_prerequisites, checkout, recipes};
use crate::hc::remote::{Target, Transfer};
use crate::relay::config::{Config, Machine};
use crate::relay::store::{Store, Submission, Task};
use std::path::Path;
use std::time::Duration;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

pub(super) struct Fixture {
  dir: tempfile::TempDir,
  pub(super) env: Env,
  pub(super) repo: String,
  pub(super) tree: String,
  pub(super) worker: Machine,
  pub(super) task: Task,
}

pub(super) fn command(env: &Env, args: &[&str]) -> Result<String, String> {
  let mut spec = Spec::new(args.iter().copied());
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(30);
  let out = run(&spec).map_err(|e| format!("{e:?}"))?;
  if out.exit_code != 0 || out.timed_out || out.truncated {
    return Err(format!("{args:?}: {}", out.stderr));
  }
  Ok(out.stdout.trim().to_owned())
}

fn environment(base: &str) -> Result<Env, String> {
  let env = Env::process()
    .with("HC_SESSION", "local")
    .with("XDG_CONFIG_HOME", &format!("{base}/c"))
    .with("XDG_DATA_HOME", &format!("{base}/d"))
    .with("XDG_STATE_HOME", &format!("{base}/s"))
    .with("XDG_RUNTIME_DIR", &format!("{base}/r"));
  std::fs::create_dir_all(format!("{base}/r")).map_err(|e| e.to_string())?;
  command(
    &env,
    &[
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
    ],
  )?;
  Ok(env)
}

fn repository(env: &Env, base: &str) -> Result<(String, String), String> {
  let repo = format!("{base}/repo");
  command(env, &["git", "init", "--quiet", &repo])?;
  std::fs::write(format!("{repo}/input"), "input\n").map_err(|e| e.to_string())?;
  command(env, &["git", "-C", &repo, "add", "input"])?;
  command(
    env,
    &[
      "git",
      "-C",
      &repo,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "base",
    ],
  )?;
  let revision = command(env, &["git", "-C", &repo, "rev-parse", "HEAD"])?;
  Ok((repo, revision))
}

impl Fixture {
  pub(super) fn new() -> Result<Self, String> {
    let dir = tempfile::Builder::new()
      .prefix("worker-hc-")
      .tempdir_in("/tmp")
      .map_err(|e| e.to_string())?;
    let base = dir.path().to_str().ok_or("invalid path")?;
    let env = environment(base)?;
    let (repo, revision) = repository(&env, base)?;
    let worker = Machine {
      name: "loop".into(),
      root: format!("{base}/worker"),
      agents: vec!["codex".into()],
      capacity: 1,
      draining: false,
    };
    std::fs::create_dir_all(format!("{}/tasks", worker.root)).map_err(|e| e.to_string())?;
    let mut config = Config::default();
    config.machines.insert(worker.name.clone(), worker.clone());
    let store = Store::new(&dir.path().join("state"));
    store.submit(
      Submission {
        key: "task".into(),
        origin: repo.clone(),
        base: revision,
        brief: "work".into(),
        agent: "codex".into(),
        argv: vec!["codex".into()],
        machine: None,
        group: None,
      },
      b"input",
    )?;
    let task = store.reserve("task", &config, 1)?.ok_or("not reserved")?;
    let tree = format!(
      "{}/tasks/{}",
      worker.root,
      task.attempt.as_deref().ok_or("missing attempt")?
    );
    Ok(Self {
      dir,
      env,
      repo,
      tree,
      worker,
      task,
    })
  }

  fn transfer<'a>(&'a self, repo: &'a str) -> Transfer<'a> {
    Transfer {
      target: Target {
        machine: "loop",
        repo,
      },
      env: &self.env,
      run: "task",
    }
  }

  pub(super) fn setup(&self) -> Result<(), String> {
    checkout(&self.transfer(&self.repo), &self.task, &self.tree)
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

#[test]
fn checkout_reuses_clean_input_but_rejects_dirty_or_changed_head() {
  let fixture = Fixture::new().unwrap();
  fixture.setup().unwrap();
  fixture.setup().unwrap();
  assert_eq!(
    command(
      &fixture.env,
      &["git", "-C", &fixture.tree, "rev-parse", "HEAD"]
    )
    .unwrap(),
    fixture.task.request.base
  );
  std::fs::write(Path::new(&fixture.tree).join("input"), "changed\n").unwrap();
  assert!(
    fixture
      .setup()
      .unwrap_err()
      .contains("differs from the input commit")
  );
  command(
    &fixture.env,
    &[
      "git",
      "-C",
      &fixture.tree,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qam",
      "changed",
    ],
  )
  .unwrap();
  assert!(
    fixture
      .setup()
      .unwrap_err()
      .contains("differs from the input commit")
  );
}

#[test]
fn recipes_run_as_argv_once_and_interrupted_intent_never_replays() {
  let fixture = Fixture::new().unwrap();
  fixture.setup().unwrap();
  let root = fixture.dir.path().join("relay");
  let attempt = fixture.task.attempt.as_deref().unwrap();
  let argv = vec![vec![
    "mkdir".to_owned(),
    "literal;touch injected".to_owned(),
  ]];
  recipes(
    &root,
    attempt,
    &fixture.transfer(&fixture.tree),
    Some(&argv),
  )
  .unwrap();
  recipes(
    &root,
    attempt,
    &fixture.transfer(&fixture.tree),
    Some(&argv),
  )
  .unwrap();
  assert!(
    Path::new(&fixture.tree)
      .join("literal;touch injected")
      .is_dir()
  );
  assert!(!Path::new(&fixture.tree).join("injected").exists());
  let mut interrupted = argv;
  interrupted.push(vec!["mkdir".into(), "must-not-run".into()]);
  std::fs::write(
    root.join("setup").join(attempt).join("1.intent"),
    serde_json::to_vec(&interrupted[1]).unwrap(),
  )
  .unwrap();
  assert!(
    recipes(
      &root,
      attempt,
      &fixture.transfer(&fixture.tree),
      Some(&interrupted)
    )
    .unwrap_err()
    .contains("uncertain recipe")
  );
  assert!(!Path::new(&fixture.tree).join("must-not-run").exists());
}

#[test]
fn rust_checkout_checks_the_real_toolchain_before_recipes() {
  let fixture = Fixture::new().unwrap();
  fixture.setup().unwrap();
  std::fs::write(Path::new(&fixture.tree).join("Cargo.toml"), "[workspace]\n").unwrap();
  build_prerequisites(&fixture.transfer(&fixture.tree)).unwrap();
}
