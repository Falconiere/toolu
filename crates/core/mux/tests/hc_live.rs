//! Real hc sessions, isolated from the user's core and desktop.
use std::time::Duration;
use toolu_mux::hc::remote::{self, Target, Transfer};
use toolu_mux::hc::{Client, Launch, address, require_owner};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

struct Fixture {
  dir: tempfile::TempDir,
  env: Env,
  remote: String,
}

impl Fixture {
  fn new() -> Result<Self, String> {
    let dir = tempfile::Builder::new()
      .prefix("hc-")
      .tempdir_in("/tmp")
      .map_err(|error| error.to_string())?;
    let root = dir.path().to_str().ok_or("non-UTF-8 temporary path")?;
    let env = Env::process()
      .with("HC_SESSION", "local")
      .with("XDG_CONFIG_HOME", &format!("{root}/c"))
      .with("XDG_DATA_HOME", &format!("{root}/d"))
      .with("XDG_STATE_HOME", &format!("{root}/s"))
      .with("XDG_RUNTIME_DIR", &format!("{root}/r"));
    std::fs::create_dir_all(format!("{root}/r")).map_err(|error| error.to_string())?;
    let fixture = Self {
      dir,
      env,
      remote: "remote".into(),
    };
    fixture.command(&[
      "hc",
      "machine",
      "add",
      "--name",
      "loop",
      "--via",
      "/bin/sh -c",
      "--session",
      "remote",
      "--temp",
    ])?;
    Ok(fixture)
  }
  fn command(&self, args: &[&str]) -> Result<String, String> {
    let mut spec = Spec::new(args.iter().copied());
    spec.env = Some(self.env.clone());
    spec.timeout = Duration::from_secs(60);
    let out = run(&spec).map_err(|error| format!("{error:?}"))?;
    if out.exit_code != 0 || out.timed_out || out.truncated {
      return Err(format!("{args:?}: {}", out.stderr));
    }
    Ok(out.stdout.trim().to_owned())
  }
  fn git(&self, repo: &str, parts: &[&str]) -> Result<String, String> {
    let mut argv = vec!["git", "-C", repo];
    argv.extend_from_slice(parts);
    self.command(&argv)
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    for session in ["local", self.remote.as_str()] {
      let mut spec = Spec::new(["hc", "server", "stop"]);
      spec.env = Some(self.env.clone().with("HC_SESSION", session));
      let _stopped = run(&spec);
    }
  }
}

#[test]
fn real_hc_launch_send_and_owner_checked_cleanup() {
  let fixture = Fixture::new().unwrap();
  let client = Client::new(&fixture.env);
  let argv = vec![
    "sh".into(),
    "-c".into(),
    "while read line; do printf 'seen:%s\\n' \"$line\"; done".into(),
  ];
  let cwd = fixture.dir.path().to_str().unwrap();
  let terminal = client
    .launch(&Launch {
      name: "owned",
      cwd,
      owner: "toolu.owner=test",
      machine: None,
      argv: &argv,
    })
    .unwrap();
  require_owner(&terminal, "test").unwrap();
  let target = address(&terminal).unwrap();
  assert!(client.kill_owned(target, "wrong").is_err());
  client.send(target, "hello").unwrap();
  let wait = client
    .call(&["wait", target, "--timeout", "1"], Duration::from_secs(10))
    .unwrap();
  assert_eq!(wait["reason"], "timeout");
  let screen = client
    .call(&["read", target], Duration::from_secs(30))
    .unwrap();
  assert!(screen["text"].as_str().unwrap().contains("seen:hello"));
  client.kill_owned(target, "test").unwrap();
  assert_eq!(client.get(target).unwrap()["state"], "finished");
}

#[test]
fn real_hc_transfers_binary_bundles_and_recovers() {
  let fixture = Fixture::new().unwrap();
  let source = tempfile::tempdir_in(fixture.dir.path()).unwrap();
  let destination = tempfile::tempdir_in(fixture.dir.path()).unwrap();
  let src = source.path().to_str().unwrap();
  let dst = destination.path().to_str().unwrap();
  for repo in [src, dst] {
    fixture.git(repo, &["init", "-q"]).unwrap();
    fixture.git(repo, &["config", "user.name", "Test"]).unwrap();
    fixture
      .git(repo, &["config", "user.email", "test@example.invalid"])
      .unwrap();
  }
  std::fs::write(source.path().join("input"), b"snapshot\0binary").unwrap();
  fixture.git(src, &["add", "input"]).unwrap();
  fixture.git(src, &["commit", "-qm", "snapshot"]).unwrap();
  fixture
    .git(dst, &["remote", "add", "origin", "example-origin"])
    .unwrap();
  let sha = fixture.git(src, &["rev-parse", "HEAD"]).unwrap();
  let mut bytes = remote::export_recover(&fixture.env, src, "transfer", &sha).unwrap();
  let transport = Transfer {
    target: Target::new(Some("loop"), Some(dst)).unwrap().unwrap(),
    env: &fixture.env,
    run: "transfer",
  };
  transport.upload("input", &sha, &bytes).unwrap();
  transport.upload("input", &sha, &bytes).unwrap();
  assert_eq!(
    transport
      .git(&["rev-parse", "refs/toolu-transfer/transfer/snapshot"])
      .unwrap(),
    sha
  );
  bytes.push(0);
  assert!(transport.upload("input", &sha, &bytes).is_err());
  transport.write("brief", b"hello\0world").unwrap();
  assert_eq!(transport.read("brief").unwrap(), b"hello\0world");
  assert!(transport.write("brief", b"other").is_err());
  let output = Transfer {
    target: Target::new(Some("loop"), Some(dst)).unwrap().unwrap(),
    env: &fixture.env,
    run: "output",
  };
  let result = output.export_recover("bundle", &sha).unwrap();
  assert_eq!(output.export_recover("bundle", &sha).unwrap(), result);
  remote::import_recover(&fixture.env, src, "output", &sha, &result).unwrap();

  direct_round_trip(&fixture, src, dst, &sha).unwrap();
}

fn direct_round_trip(fixture: &Fixture, src: &str, dst: &str, sha: &str) -> Result<(), String> {
  let direct = Transfer {
    target: Target::new(Some("loop"), Some(dst))
      .map_err(|error| format!("{error:?}"))?
      .ok_or("missing target")?,
    env: &fixture.env,
    run: "direct",
  };
  direct.allocate().map_err(|error| format!("{error:?}"))?;
  let result = direct
    .export("bundle", sha)
    .map_err(|error| format!("{error:?}"))?;
  remote::import(&fixture.env, src, "direct", sha, &result)
    .map_err(|error| format!("{error:?}"))?;
  if fixture.git(src, &["show", "refs/toolu-transfer/output/snapshot:input"])? != "snapshot\0binary"
  {
    return Err("imported bytes differ".into());
  }
  Ok(())
}
