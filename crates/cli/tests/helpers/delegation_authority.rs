use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub(crate) struct Fixture {
  dir: tempfile::TempDir,
  path: String,
}

impl Fixture {
  pub(crate) fn new() -> Result<Self, String> {
    let dir = tempfile::Builder::new()
      .prefix("authority-")
      .tempdir_in("/tmp")
      .map_err(|e| e.to_string())?;
    let bin = Path::new(env!("CARGO_BIN_EXE_toolu"))
      .parent()
      .ok_or("missing toolu parent")?;
    let path = format!(
      "{}:{}",
      bin.display(),
      std::env::var("PATH").map_err(|e| e.to_string())?
    );
    let fixture = Self { dir, path };
    for node in ["controller", "alpha", "beta"] {
      for name in ["c", "d", "s", "r"] {
        std::fs::create_dir_all(fixture.dir.path().join(node).join(name))
          .map_err(|e| e.to_string())?;
      }
    }
    for name in ["alpha", "beta"] {
      fixture.add_machine(name)?;
    }
    Ok(fixture)
  }

  fn env(&self, command: &mut Command, node: &str, session: &str) {
    let base = self.dir.path().join(node);
    command
      .env_clear()
      .env("HOME", self.dir.path())
      .env("PATH", &self.path)
      .env("HC_SESSION", session)
      .env("XDG_CONFIG_HOME", base.join("c"))
      .env("XDG_DATA_HOME", base.join("d"))
      .env("XDG_STATE_HOME", base.join("s"))
      .env("XDG_RUNTIME_DIR", base.join("r"));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
  }

  fn add_machine(&self, node: &str) -> Result<(), String> {
    let base = self.dir.path().join(node);
    let via = format!(
      "env -u TOOLU_CONFIG_DIR XDG_CONFIG_HOME={0}/c XDG_DATA_HOME={0}/d XDG_STATE_HOME={0}/s XDG_RUNTIME_DIR={0}/r PATH={1} /bin/sh -c",
      base.display(),
      self.path
    );
    self
      .hc(
        &[
          "machine",
          "add",
          "--name",
          node,
          "--via",
          &via,
          "--session",
          node,
          "--temp",
        ],
        None,
      )
      .and_then(ok)
      .map(|_| ())
  }

  pub(crate) fn cli(&self, controller: &str, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toolu"));
    self.env(&mut command, "controller", "controller");
    command
      .args(["--json", "--config-dir"])
      .arg(self.dir.path().join(controller))
      .args(args);
    command.output().map_err(|e| e.to_string())
  }

  pub(crate) fn hc(&self, args: &[&str], input: Option<&Value>) -> Result<Output, String> {
    let mut command = Command::new("hc");
    self.env(&mut command, "controller", "controller");
    command
      .args(args)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());
    if let Some(value) = input {
      let mut child = command
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
      child
        .stdin
        .take()
        .ok_or("missing stdin")?
        .write_all(value.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
      child.wait_with_output().map_err(|e| e.to_string())
    } else {
      command.output().map_err(|e| e.to_string())
    }
  }

  pub(crate) fn rpc(&self, node: &str, request: &Value) -> Result<Output, String> {
    self.hc(
      &[
        "exec", "--on", node, "-i", "--", "toolu", "--json", "relay", "receive",
      ],
      Some(request),
    )
  }

  pub(crate) fn config(&self, node: &str) -> Result<Value, String> {
    let output = ok(self.rpc(node, &json!({"op":"configuration"}))?)?;
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
  }

  pub(crate) fn controller_root(&self, name: &str) -> PathBuf {
    self.dir.path().join(name)
  }

  pub(crate) fn authority_path(&self, node: &str) -> PathBuf {
    self
      .dir
      .path()
      .join(node)
      .join("c/toolu/delegation/authority.json")
  }

  pub(crate) fn node_config(&self, node: &str) -> PathBuf {
    self.authority_path(node).with_file_name("config.json")
  }
}

impl Fixture {
  fn stop_core(&self, node: &str) -> Result<bool, String> {
    let mut command = Command::new("hc");
    self.env(&mut command, node, node);
    let output = ok(
      command
        .args(["server", "stop", "--json"])
        .output()
        .map_err(|e| e.to_string())?,
    )?;
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    value
      .get("stopped")
      .and_then(Value::as_bool)
      .ok_or_else(|| "missing stop status".into())
  }

  fn stop_cores(&self) -> Result<(), String> {
    for node in ["controller", "alpha", "beta"] {
      self.stop_core(node)?;
    }
    Ok(())
  }

  pub(crate) fn git(&self, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    self.env(&mut command, "controller", "controller");
    let output = ok(command.args(args).output().map_err(|e| e.to_string())?)?;
    Ok(
      String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())?
        .trim_end()
        .into(),
    )
  }

  pub(crate) fn seed_result(
    &self,
    node: &str,
    key: &str,
    repo: &str,
  ) -> Result<(String, String), String> {
    let (worker, commit) = self.result_commit(repo)?;
    let worker = worker.to_str().ok_or("invalid worker path")?;
    let root = self
      .node_config(node)
      .parent()
      .ok_or("missing relay root")?
      .to_path_buf();
    let store = toolu_mux::relay::store::Store::new(&root);
    let config = toolu_mux::relay::config::load(&root)?;
    let task = store
      .reserve(key, &config, 0)?
      .ok_or("task was not reserved")?;
    let attempt = task.attempt.ok_or("missing attempt")?;
    let reference = format!("refs/toolu-transfer/{attempt}/snapshot");
    self.git(&["-C", worker, "update-ref", &reference, &commit])?;
    let bundle = self.controller_root("result.bundle");
    self.git(&[
      "-C",
      worker,
      "bundle",
      "create",
      bundle.to_str().ok_or("invalid bundle path")?,
      &reference,
    ])?;
    store.report(
      key,
      &attempt,
      &commit,
      &std::fs::read(bundle).map_err(|e| e.to_string())?,
    )?;
    Ok((attempt, commit))
  }

  fn result_commit(&self, repo: &str) -> Result<(PathBuf, String), String> {
    let path = self.controller_root("result-worker");
    let worker = path.to_str().ok_or("invalid worker path")?;
    self.git(&["clone", "-q", repo, worker])?;
    std::fs::write(Path::new(worker).join("result"), "finished\n").map_err(|e| e.to_string())?;
    self.git(&["-C", worker, "add", "result"])?;
    self.git(&[
      "-C",
      worker,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "result",
    ])?;
    let commit = self.git(&["-C", worker, "rev-parse", "HEAD"])?;
    Ok((path, commit))
  }

  pub(crate) fn repo(&self) -> Result<(String, String), String> {
    let repo = self.controller_root("repo");
    let origin = self.controller_root("origin.git");
    let repo_text = repo.to_str().ok_or("invalid repo path")?;
    let origin_text = origin.to_str().ok_or("invalid origin path")?;
    self.git(&["init", "--bare", "--quiet", origin_text])?;
    self.git(&["init", "--quiet", repo_text])?;
    std::fs::write(repo.join("input"), "committed input\n").map_err(|e| e.to_string())?;
    self.git(&["-C", repo_text, "add", "input"])?;
    self.git(&[
      "-C",
      repo_text,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-qm",
      "initial",
    ])?;
    self.git(&["-C", repo_text, "remote", "add", "origin", origin_text])?;
    let brief = self.controller_root("brief");
    std::fs::write(&brief, "Fix the supplied task.\n").map_err(|e| e.to_string())?;
    let brief = brief.to_str().ok_or("invalid brief path")?;
    Ok((repo_text.into(), brief.into()))
  }
}

pub(crate) fn ok(output: Output) -> Result<Output, String> {
  if output.status.success() {
    Ok(output)
  } else {
    Err(format!(
      "status {}: {} {}",
      output.status,
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    ))
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    for node in ["alpha", "beta"] {
      if let Ok(config) = self.config(node)
        && config.get("retirement").is_none_or(Value::is_null)
        && config
          .pointer("/config/relay")
          .is_some_and(Value::is_string)
      {
        let _ = self.rpc(
          node,
          &json!({"op":"retire", "expected":config.get("config"), "successor":"cleanup"}),
        );
      }
    }
    for node in ["alpha", "beta"] {
      let target = format!("{node}/toolu-relay");
      let _ = self.hc(&["wait", &target, "--timeout", "5"], None);
    }
    let _ = self.stop_cores();
  }
}

#[test]
fn isolated_node_cores_do_not_restart_during_fixture_cleanup() {
  let fixture = Fixture::new().unwrap();
  for node in ["alpha", "beta"] {
    fixture.config(node).unwrap();
  }
  fixture.stop_cores().unwrap();
  std::thread::sleep(std::time::Duration::from_secs(2));
  for node in ["alpha", "beta"] {
    assert!(
      !fixture.stop_core(node).unwrap(),
      "{node} restarted after cleanup"
    );
  }
}
