use serde_json::{Value, json};
use std::{
  io::Write as _,
  path::{Path, PathBuf},
  process::{Command, Output, Stdio},
};

pub(crate) struct Fixture {
  dir: tempfile::TempDir,
  path: String,
  pub(crate) config: Value,
}
impl Fixture {
  pub(crate) fn new() -> Self {
    let dir = tempfile::Builder::new()
      .prefix("peers-cli-")
      .tempdir_in("/tmp")
      .unwrap();
    std::fs::create_dir_all(dir.path().join("runtime")).unwrap();
    std::fs::create_dir_all(dir.path().join("bin")).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_toolu"), dir.path().join("bin/toolu")).unwrap();
    let path = format!(
      "{}:{}",
      dir.path().join("bin").display(),
      std::env::var("PATH").unwrap_or_default()
    );
    let machine = |name: &str| json!({"name":name,"root":format!("/tmp/{name}"),"agents":["codex","claude"],"capacity":1,"draining":false});
    let config = json!({"version":1,"relay":"loop","generation":"foundation","machines":{"one":machine("one"),"two":machine("two")},"recipes":{}});
    let fixture = Self { dir, path, config };
    fixture.initialize();
    fixture
  }
  fn command(&self, program: &str, session: &str, config: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd
      .env("PATH", &self.path)
      .env("HOME", self.dir.path())
      .env("HC_SESSION", session)
      .env("TOOLU_CONFIG_DIR", config)
      .env("XDG_CONFIG_HOME", self.dir.path().join("config"))
      .env("XDG_DATA_HOME", self.dir.path().join("data"))
      .env("XDG_STATE_HOME", self.dir.path().join("state"))
      .env("XDG_RUNTIME_DIR", self.dir.path().join("runtime"));
    cmd
  }
  fn hc(&self, session: &str, args: &[&str]) {
    let output = self
      .command("hc", session, &self.root("relay"))
      .args(args)
      .output()
      .unwrap();
    assert!(
      output.status.success(),
      "hc {args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }
  fn initialize(&self) {
    self.hc("peerremote", &["server", "start"]);
    self.hc("peerlocal", &["server", "start"]);
    self.hc(
      "peerlocal",
      &[
        "machine",
        "add",
        "--name",
        "loop",
        "--via",
        "/bin/sh -c",
        "--session",
        "peerremote",
        "--temp",
      ],
    );
    self.enroll();
    self.controller_configs();
  }
  fn enroll(&self) {
    let payload = json!({"op":"enroll","expected":{"version":1,"relay":null,"generation":null,"machines":{},"recipes":{}},"config":self.config});
    let mut child = self
      .command(
        env!("CARGO_BIN_EXE_toolu"),
        "peerremote",
        &self.root("relay"),
      )
      .args(["--json", "relay", "receive"])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    child
      .stdin
      .take()
      .unwrap()
      .write_all(payload.to_string().as_bytes())
      .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
      output.status.success(),
      "{}",
      String::from_utf8_lossy(&output.stderr)
    );
  }
  fn controller_configs(&self) {
    for name in ["alice", "bob", "eve"] {
      let path = self.root(name).join("delegation/config.json");
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(path, self.config.to_string()).unwrap();
    }
    std::fs::write(
      self.dir.path().join("key"),
      "0123456789abcdef0123456789abcdef",
    )
    .unwrap();
    std::fs::write(
      self.dir.path().join("bad-key"),
      "abcdef0123456789abcdef0123456789",
    )
    .unwrap();
  }
}
impl Fixture {
  pub(crate) fn root(&self, name: &str) -> PathBuf {
    self.dir.path().join(name)
  }
  pub(crate) fn key(&self, name: &str) -> String {
    self.dir.path().join(name).to_string_lossy().into_owned()
  }
  pub(crate) fn cli_output(&self, name: &str, args: &[&str], offline: bool) -> Output {
    let mut cmd = self.command(env!("CARGO_BIN_EXE_toolu"), "peerlocal", &self.root(name));
    if offline {
      cmd.env("PATH", "/definitely/absent");
    }
    cmd
      .args(["--json", "--config-dir"])
      .arg(self.root(name))
      .args(args)
      .output()
      .unwrap()
  }
  pub(crate) fn cli(&self, name: &str, args: &[&str]) -> Value {
    let output = self.cli_output(name, args, false);
    assert!(
      output.status.success(),
      "{args:?}: stdout={} stderr={}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
  }
  pub(crate) fn join(&self, name: &str, key: &str) -> Value {
    let (machine, agent) = if name == "bob" {
      ("two", "claude")
    } else {
      ("one", "codex")
    };
    self.cli(
      name,
      &[
        "peers",
        "join",
        "--group",
        "team",
        "--machine",
        machine,
        "--agent",
        agent,
        "--key-file",
        &self.key(key),
      ],
    )
  }
  pub(crate) fn peer(&self, name: &str, id: &str, args: &[&str]) -> Value {
    let mut command = vec!["peers", "--session", id];
    command.extend_from_slice(args);
    self.cli(name, &command)
  }
  pub(crate) fn rpc(&self, input: &Value) -> Value {
    let mut child = self
      .command("hc", "peerlocal", &self.root("alice"))
      .args([
        "exec", "--on", "loop", "-i", "--", "toolu", "--json", "relay", "receive",
      ])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    child
      .stdin
      .take()
      .unwrap()
      .write_all(input.to_string().as_bytes())
      .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
      output.status.success(),
      "{}",
      String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
  }
}
impl Drop for Fixture {
  fn drop(&mut self) {
    for session in ["peerlocal", "peerremote"] {
      let _ = self
        .command("hc", session, &self.root("relay"))
        .args(["server", "stop"])
        .output();
    }
  }
}
