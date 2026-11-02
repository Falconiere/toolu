use super::super::repository;
use crate::relay::config::Machine;
use crate::relay::store::{Store, Submission, Task};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

fn command(env: &Env, args: &[&str], input: &[u8]) -> String {
  let mut spec = Spec::new(args.iter().copied());
  spec.env = Some(env.clone());
  spec.stdin = input.to_vec();
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "{}", output.stderr);
  String::from_utf8(output.stdout_bytes)
    .unwrap()
    .trim()
    .to_owned()
}

struct Fixture {
  _dir: tempfile::TempDir,
  env: Env,
  worker: Machine,
  origin: String,
  repo: String,
  task: Task,
}

impl Fixture {
  fn new() -> Self {
    let dir = tempfile::Builder::new()
      .prefix("clone-hc-")
      .tempdir_in("/tmp")
      .unwrap();
    let base = dir.path().to_str().unwrap();
    let env = Env::process()
      .with("HC_SESSION", "clone-test")
      .with("XDG_CONFIG_HOME", &format!("{base}/c"))
      .with("XDG_DATA_HOME", &format!("{base}/d"))
      .with("XDG_STATE_HOME", &format!("{base}/s"))
      .with("XDG_RUNTIME_DIR", &format!("{base}/r"));
    std::fs::create_dir_all(format!("{base}/r")).unwrap();
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
      &[],
    );
    let origin = format!("{base}/origin");
    command(&env, &["git", "init", "--bare", "-q", &origin], &[]);
    let hash = command(&env, &["git", "hash-object", "--stdin"], origin.as_bytes());
    let worker = Machine {
      name: "loop".into(),
      root: format!("{base}/worker"),
      agents: vec!["codex".into()],
      capacity: 1,
      draining: false,
    };
    let task = reserve(&dir, &worker, &origin);
    let repo = format!("{}/repos/{hash}", worker.root);
    Self {
      _dir: dir,
      env,
      worker,
      origin,
      repo,
      task,
    }
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    for session in ["clone-test", "remotea"] {
      let mut spec = Spec::new(["hc", "server", "stop"]);
      spec.env = Some(self.env.clone().with("HC_SESSION", session));
      let _ = run(&spec);
    }
  }
}

fn reserve(dir: &tempfile::TempDir, worker: &Machine, origin: &str) -> Task {
  let mut cfg = crate::relay::config::Config::default();
  cfg.machines.insert(worker.name.clone(), worker.clone());
  let store = Store::new(&dir.path().join("state"));
  let request = Submission {
    key: "clone".into(),
    origin: origin.into(),
    base: "a".repeat(40),
    brief: "work".into(),
    agent: worker.agents[0].clone(),
    argv: worker.agents.clone(),
    machine: None,
    group: None,
  };
  store.submit(request, b"input").unwrap();
  store.reserve("clone", &cfg, 1).unwrap().unwrap()
}

#[test]
fn interrupted_clones_are_preserved_and_foreign_repositories_are_refused() {
  let f = Fixture::new();
  std::fs::create_dir_all(&f.repo).unwrap();
  std::fs::write(format!("{}/unfinished", f.repo), b"preserve").unwrap();
  let stage = format!("{}.{}.clone", f.repo, f.task.attempt.as_deref().unwrap());
  std::fs::create_dir(&stage).unwrap();
  std::fs::write(format!("{stage}/partial"), b"stage").unwrap();
  assert_eq!(repository(&f.env, &f.task, &f.worker).unwrap(), f.repo);
  assert_eq!(
    command(
      &f.env,
      &["git", "-C", &f.repo, "remote", "get-url", "origin"],
      &[]
    ),
    f.origin
  );
  let names: Vec<_> = std::fs::read_dir(format!("{}/repos", f.worker.root))
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .collect();
  assert!(
    names
      .iter()
      .any(|path| std::fs::read(path.join("unfinished")).ok().as_deref() == Some(b"preserve"))
  );
  assert!(
    names
      .iter()
      .any(|path| std::fs::read(path.join("partial")).ok().as_deref() == Some(b"stage"))
  );
  verify_foreign_refused(&f);
}

fn verify_foreign_refused(f: &Fixture) {
  command(
    &f.env,
    &[
      "git",
      "-C",
      &f.repo,
      "remote",
      "set-url",
      "origin",
      "/different",
    ],
    &[],
  );
  assert!(repository(&f.env, &f.task, &f.worker).is_err());
  assert_eq!(
    command(
      &f.env,
      &["git", "-C", &f.repo, "remote", "get-url", "origin"],
      &[]
    ),
    "/different"
  );
}

#[test]
fn unreadable_mirror_with_a_registered_worktree_is_never_moved() {
  let f = Fixture::new();
  repository(&f.env, &f.task, &f.worker).unwrap();
  std::fs::write(format!("{}/file", f.repo), b"input").unwrap();
  command(&f.env, &["git", "-C", &f.repo, "add", "file"], &[]);
  command(
    &f.env,
    &[
      "git",
      "-C",
      &f.repo,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "test",
    ],
    &[],
  );
  let tree = format!("{}/live", f.worker.root);
  command(
    &f.env,
    &[
      "git", "-C", &f.repo, "worktree", "add", "--detach", &tree, "HEAD",
    ],
    &[],
  );
  let config = format!("{}/.git/config", f.repo);
  let saved = std::fs::read(&config).unwrap();
  std::fs::write(&config, b"not valid git configuration\n").unwrap();
  assert!(repository(&f.env, &f.task, &f.worker).is_err());
  assert!(std::path::Path::new(&config).exists());
  std::fs::write(&config, saved).unwrap();
  assert_eq!(
    command(
      &f.env,
      &["git", "-C", &tree, "rev-parse", "--show-toplevel"],
      &[]
    ),
    tree
  );
}
