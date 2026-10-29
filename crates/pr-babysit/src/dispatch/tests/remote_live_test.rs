use super::*;

struct HcRound {
  dir: tempfile::TempDir,
  env: toolu_runtime::env::Env,
}

impl HcRound {
  fn new(sb: &crate::dispatch::sandbox::Sandbox) -> Self {
    let dir = tempfile::Builder::new()
      .prefix("pb-")
      .tempdir_in("/tmp")
      .unwrap();
    let root = dir.path().display().to_string();
    std::fs::create_dir_all(format!("{root}/r")).unwrap();
    let path = format!(
      "{}:{}",
      sb.env.get("PATH").unwrap(),
      std::env::var("PATH").unwrap_or_default()
    );
    let env = sb
      .env
      .clone()
      .with("PATH", &path)
      .with("HC_SESSION", "local")
      .with("XDG_CONFIG_HOME", &format!("{root}/c"))
      .with("XDG_DATA_HOME", &format!("{root}/d"))
      .with("XDG_STATE_HOME", &format!("{root}/s"))
      .with("XDG_RUNTIME_DIR", &format!("{root}/r"))
      .with("TOOLU_EPIC_MULTIPLEXER", "herdr-app");
    let fixture = Self { dir, env };
    fixture.hc(&[
      "machine",
      "add",
      "--name",
      "loop",
      "--via",
      "/bin/sh -c",
      "--session",
      "remote",
      "--temp",
    ]);
    fixture
  }

  fn hc(&self, args: &[&str]) {
    let mut spec =
      toolu_runtime::process::Spec::new(std::iter::once("hc").chain(args.iter().copied()));
    spec.env = Some(self.env.clone());
    let out = toolu_runtime::process::run(&spec).unwrap();
    assert_eq!(out.exit_code, 0, "{}", out.stderr);
  }
}

impl Drop for HcRound {
  fn drop(&mut self) {
    for session in ["local", "remote"] {
      let mut spec = toolu_runtime::process::Spec::new(["hc", "server", "stop"]);
      spec.env = Some(self.env.clone().with("HC_SESSION", session));
      let _ = toolu_runtime::process::run(&spec);
    }
  }
}

fn setup_round(sb: &crate::dispatch::sandbox::Sandbox, hc: &HcRound, repo: &Path) -> Value {
  crate::dispatch::sandbox::git(
    &hc.env,
    &sb.repo,
    &[
      "clone",
      "--quiet",
      sb.origin.to_str().unwrap(),
      repo.to_str().unwrap(),
    ],
  );
  let mut options = sb.options(false);
  options.env = &hc.env;
  options.toolu = Path::new("/usr/bin/git");
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  let local = dispatcher.worktree(&sb.root, "feat/fix").unwrap();
  dispatcher
    .setup_remote("loop", repo.to_str().unwrap(), &[], "main")
    .unwrap();
  local
}

fn record_round(sb: &crate::dispatch::sandbox::Sandbox, hc: &HcRound) {
  let slot = crate::writes::Slot {
    path: &sb.state,
    env: &hc.env,
    now: "2026-10-29T00:00:00Z",
  };
  crate::record::record(
    &slot,
    &crate::record::Entry::Round {
      had_rejection: false,
      fix_pushed: false,
    },
  )
  .unwrap();
}

fn make_failed_report(dispatcher: &mut Dispatcher<'_>, hc: &HcRound) -> (Value, Value) {
  let data = dispatcher.prepare_remote_group(1).unwrap();
  let remote = dispatcher.remote().unwrap().clone();
  let path = field(&remote, "path").unwrap();
  std::fs::write(Path::new(path).join("uncommitted"), "keep").unwrap();
  let report = json!({"version":1,"status":"failed","note":"tests failed", "runId":data["run"], "expectedBase":data["base"], "commit":data["base"]});
  let transport = transfer(&hc.env, &remote, field(&data, "run").unwrap()).unwrap();
  transport
    .exec(
      &["tee", field(&data, "report").unwrap()],
      report.to_string().as_bytes(),
    )
    .unwrap();
  let client = toolu_mux::hc::Client::new(&hc.env);
  let argv = vec!["sh".to_owned(), "-c".to_owned(), "exit 0".to_owned()];
  let terminal = client
    .launch(&toolu_mux::hc::Launch {
      name: "pb-failed",
      cwd: path,
      owner: "toolu.owner=failed-test",
      machine: Some("loop"),
      argv: &argv,
    })
    .unwrap();
  let address = toolu_mux::hc::address(&terminal).unwrap();
  client
    .call(
      &["wait", address, "--timeout", "10"],
      Duration::from_secs(20),
    )
    .unwrap();
  dispatcher.patch_group(1, &json!({"agent":address, "hcAddress":address, "hcName":"pb-failed", "hcOwner":"toolu.owner=failed-test", "hcSession":"local"})).unwrap();
  (data, remote)
}

fn assert_rotated(dispatcher: &Dispatcher<'_>, first: &Value, second: &Value, head: &str) {
  assert_ne!(field(first, "run").unwrap(), field(second, "run").unwrap());
  assert_eq!(field(second, "expectedBase").unwrap(), head);
  assert_ne!(
    field(first, "path").unwrap(),
    field(second, "path").unwrap()
  );
  assert_eq!(
    std::fs::read_to_string(Path::new(field(first, "path").unwrap()).join("uncommitted")).unwrap(),
    "preserve"
  );
  assert_eq!(
    dispatcher.state["remoteHistory"][0]["remote"]["run"],
    first["run"]
  );
  assert_eq!(
    dispatcher.state["remoteHistory"][0]["remote"]["path"],
    first["path"]
  );
  assert_eq!(
    dispatcher.state["remoteHistory"][0]["fixer"]["status"],
    "done"
  );
}

#[test]
fn completed_round_starts_fresh_at_imported_head_and_preserves_old_work() {
  let sb = crate::dispatch::sandbox::sandbox();
  let hc = HcRound::new(&sb);
  let remote_repo = hc.dir.path().join("repo");
  let remote_text = remote_repo.to_str().unwrap();
  let created = setup_round(&sb, &hc, &remote_repo);
  let mut options = sb.options(false);
  options.env = &hc.env;
  options.toolu = Path::new("/usr/bin/git");
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  let first = dispatcher.remote().unwrap().clone();
  let old_path = Path::new(field(&first, "path").unwrap());
  std::fs::write(old_path.join("uncommitted"), "preserve").unwrap();
  let local = Path::new(field(&created, "path").unwrap());
  std::fs::write(local.join("fix"), "committed fix").unwrap();
  crate::dispatch::sandbox::git(&hc.env, local, &["add", "fix"]);
  crate::dispatch::sandbox::git(&hc.env, local, &["commit", "--quiet", "-m", "fix"]);
  let head = crate::dispatch::sandbox::git(&hc.env, local, &["rev-parse", "HEAD"]);
  dispatcher
    .save(|state| {
      state["herdrWorktree"]["remote"]["setupPending"] = json!(false);
      state["fixer"] = json!({"status":"done", "groups":[{"seq":1,"status":"done"}]});
    })
    .unwrap();
  record_round(&sb, &hc);
  dispatcher = Dispatcher::new(&options).unwrap();
  dispatcher
    .setup_remote("loop", remote_text, &[], "main")
    .unwrap();
  let second = dispatcher.remote().unwrap().clone();
  assert_rotated(&dispatcher, &first, &second, &head);
  dispatcher
    .setup_remote("loop", remote_text, &[], "main")
    .unwrap();
  assert_eq!(dispatcher.remote().unwrap(), &second);
  assert_eq!(
    dispatcher.state["remoteHistory"].as_array().unwrap().len(),
    1
  );
}

fn assert_failed(dispatcher: &Dispatcher<'_>, result: &Value, path: &str) {
  assert_eq!(result["status"], "failed");
  assert_eq!(result["reason"], "reported_failed");
  assert_eq!(dispatcher.group(1).unwrap()["error"], "tests failed");
  assert!(
    dispatcher
      .group(1)
      .unwrap()
      .get("remoteExportStarted")
      .is_none()
  );
  assert_eq!(
    std::fs::read_to_string(Path::new(path).join("uncommitted")).unwrap(),
    "keep"
  );
}

#[test]
fn failed_remote_report_settles_without_importing_or_removing_dirty_work() {
  let sb = crate::dispatch::sandbox::sandbox();
  let hc = HcRound::new(&sb);
  let repo = hc.dir.path().join("failed-repo");
  let repo_text = repo.to_str().unwrap();
  let local = setup_round(&sb, &hc, &repo);
  let mut options = sb.options(false);
  options.env = &hc.env;
  options.toolu = Path::new("/usr/bin/git");
  let mut dispatcher = Dispatcher::new(&options).unwrap();
  dispatcher
    .save(|state| {
      state["herdrWorktree"]["remote"]["setupPending"] = json!(false);
      state["fixer"] =
        json!({"status":"running", "current":1, "groups":[{"seq":1,"status":"running"}]});
    })
    .unwrap();
  let (data, remote) = make_failed_report(&mut dispatcher, &hc);
  let path = field(&remote, "path").unwrap();
  let result = dispatcher.wait(20).unwrap();
  assert_failed(&dispatcher, &result, path);
  let head = crate::dispatch::sandbox::git(
    &hc.env,
    Path::new(field(&local, "path").unwrap()),
    &["rev-parse", "HEAD"],
  );
  assert_eq!(head, data["base"].as_str().unwrap());
  record_round(&sb, &hc);
  dispatcher = Dispatcher::new(&options).unwrap();
  assert!(
    dispatcher
      .setup_remote("other", repo_text, &[], "main")
      .is_err()
  );
  dispatcher
    .setup_remote("loop", repo_text, &[], "main")
    .unwrap();
  assert_ne!(dispatcher.remote().unwrap()["run"], remote["run"]);
  assert_eq!(
    dispatcher.state["remoteHistory"][0]["fixer"]["status"],
    "failed"
  );
  assert_eq!(
    std::fs::read_to_string(Path::new(path).join("uncommitted")).unwrap(),
    "keep"
  );
}
