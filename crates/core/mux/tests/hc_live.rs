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

#[test]
fn interrupted_remote_chunks_and_assembly_resume_without_replacing_wrong_bytes() {
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
  std::fs::write(source.path().join("file"), b"binary\0snapshot").unwrap();
  fixture.git(src, &["add", "file"]).unwrap();
  fixture.git(src, &["commit", "-qm", "one"]).unwrap();
  let sha = fixture.git(src, &["rev-parse", "HEAD"]).unwrap();
  let bytes = remote::export_recover(&fixture.env, src, "partial", &sha).unwrap();
  let transfer = Transfer {
    target: Target::new(Some("loop"), Some(dst)).unwrap().unwrap(),
    env: &fixture.env,
    run: "partial",
  };
  transfer.allocate().unwrap();
  transfer.write("bad-chunk-0", b"wrong").unwrap();
  assert!(transfer.upload("bad", &sha, &bytes).is_err());
  assert_eq!(transfer.read("bad-chunk-0").unwrap(), b"wrong");
  transfer.write("input-chunk-0", &bytes[..12]).unwrap();
  transfer.write("input", &bytes[..19]).unwrap();
  transfer.upload("input", &sha, &bytes).unwrap();
  assert_eq!(transfer.read_bundle("input").unwrap(), bytes);
  let dir = repo_transfer_dir(dst, "partial", "input");
  assert!(std::fs::read_dir(dir).unwrap().flatten().any(|entry| {
    std::fs::read(entry.path().join("previous")).is_ok_and(|prior| prior == bytes[..19])
  }));
  let mut wrong = bytes.clone();
  wrong.push(0);
  assert!(transfer.upload("input", &sha, &wrong).is_err());
  assert_eq!(transfer.read_bundle("input").unwrap(), bytes);
}

#[test]
fn remote_get_by_name_and_address_keeps_machine_qualification() {
  let fixture = Fixture::new().unwrap();
  let client = Client::new(&fixture.env);
  let argv = vec![
    "sh".into(),
    "-c".into(),
    "while read line; do :; done".into(),
  ];
  let terminal = client
    .launch(&Launch {
      name: "remote-owned",
      cwd: fixture.dir.path().to_str().unwrap(),
      owner: "toolu.owner=remote",
      machine: Some("loop"),
      argv: &argv,
    })
    .unwrap();
  let target = address(&terminal).unwrap();
  assert!(target.starts_with("loop/t"));
  assert_eq!(
    address(&client.get("loop/remote-owned").unwrap()).unwrap(),
    target
  );
  assert_eq!(address(&client.get(target).unwrap()).unwrap(), target);
  client.kill_owned(target, "remote").unwrap();
}

#[test]
fn interrupted_remote_export_resumes_from_partial_bundle() {
  let fixture = Fixture::new().unwrap();
  let repo = tempfile::tempdir_in(fixture.dir.path()).unwrap();
  let root = repo.path().to_str().unwrap();
  fixture.git(root, &["init", "-q"]).unwrap();
  fixture.git(root, &["config", "user.name", "Test"]).unwrap();
  fixture
    .git(root, &["config", "user.email", "test@example.invalid"])
    .unwrap();
  std::fs::write(repo.path().join("file"), b"snapshot").unwrap();
  fixture.git(root, &["add", "file"]).unwrap();
  fixture.git(root, &["commit", "-qm", "one"]).unwrap();
  let sha = fixture.git(root, &["rev-parse", "HEAD"]).unwrap();
  let transfer = Transfer {
    target: Target::new(Some("loop"), Some(root)).unwrap().unwrap(),
    env: &fixture.env,
    run: "export-partial",
  };
  transfer.allocate().unwrap();
  transfer
    .git(&[
      "update-ref",
      "refs/toolu-transfer/export-partial/snapshot",
      &sha,
      "",
    ])
    .unwrap();
  let reference = transfer.path("reference").unwrap();
  let parent = std::path::Path::new(&reference)
    .parent()
    .unwrap()
    .to_str()
    .unwrap();
  transfer.exec(&["mkdir", parent], &[]).unwrap();
  transfer
    .git(&[
      "bundle",
      "create",
      &reference,
      "refs/toolu-transfer/export-partial/snapshot",
    ])
    .unwrap();
  let bytes = transfer.read_bundle("reference").unwrap();
  transfer.write("result", &bytes[..19]).unwrap();
  assert_eq!(transfer.export_recover("result", &sha).unwrap(), bytes);
  assert_eq!(transfer.export_recover("result", &sha).unwrap(), bytes);
}

fn repo_transfer_dir(repo: &str, run: &str, name: &str) -> std::path::PathBuf {
  std::path::Path::new(repo)
    .join(".git/toolu-transfer")
    .join(run)
    .join(name)
}
