//! Worker notices against a live engine over the real binary and control
//! socket: `notify` journals and raises attention (AC-1), a refused
//! recipient fails the verb, and an addressed notice reaches its sibling at
//! once (AC-6).
//!
//! AC-2 (a merge prompts its waiting dependents through the debounce queue)
//! is covered by the `merge_bridge` and `schedule_prompt` unit tests: no CLI
//! verb can merge without GitHub.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The debug `toolu` binary, built when stale or missing.
fn toolu_binary() -> Result<PathBuf, String> {
  let target = std::env::var_os("CARGO_TARGET_DIR")
    .map_or_else(|| workspace_root().join("target"), PathBuf::from);
  Command::new("cargo")
    .args(["build", "--locked", "-p", "toolu-cli"])
    .arg("--manifest-path")
    .arg(workspace_root().join("Cargo.toml"))
    .arg("--target-dir")
    .arg(&target)
    .env_remove("RUSTFLAGS")
    .env_remove("CARGO_ENCODED_RUSTFLAGS")
    .status()
    .map_err(|err| format!("cargo build: {err}"))
    .and_then(|status| {
      status
        .success()
        .then_some(target.join("debug").join("toolu"))
        .ok_or_else(|| "cargo build -p toolu-cli failed".to_owned())
    })
}

struct Rig {
  tmp: tempfile::TempDir,
  resources: PathBuf,
  state: PathBuf,
  bin: PathBuf,
  child: Option<Child>,
}

impl Rig {
  fn env(&self, command: &mut Command) -> PathBuf {
    let home = self.tmp.path().join("home");
    command
      .env("HOME", &home)
      .env("TOOLU_RESOURCE_HOME", &self.resources)
      .env("TOOLU_EPIC_HERDR", "off");
    home
  }

  fn write_registry(&self) -> Result<(), String> {
    std::fs::write(
      self.resources.join("registry.json"),
      format!(
        "{{\"version\": 1, \"epics\": [{{\"key\": \"one\", \"state_dir\": \"{}\"}}]}}\n",
        self.state.display()
      ),
    )
    .map_err(|err| format!("registry: {err}"))
  }

  fn spawn_serve(&mut self) -> Result<(), String> {
    let mut command = Command::new(&self.bin);
    command.args(["epic", "serve", "--no-http"]);
    self.env(&mut command);
    command
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null());
    self.child = command
      .spawn()
      .map(Some)
      .map_err(|err| format!("serve: {err}"))?;
    Ok(())
  }

  /// Liveness, not just the socket file: a failed start can leave a stale one.
  fn wait_live(&mut self) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
      let mut probe = Command::new(&self.bin);
      probe.args(["epic", "status"]);
      self.env(&mut probe);
      let output = probe
        .output()
        .map_err(|err| format!("status probe: {err}"))?;
      if output.status.success() {
        return Ok(());
      }
      if Instant::now() >= deadline {
        return Err("engine never answered status".to_owned());
      }
      let running = self
        .child
        .as_mut()
        .ok_or_else(|| "serve child missing".to_owned())?
        .try_wait()
        .map_err(|err| format!("try_wait: {err}"))?
        .is_none();
      if !running {
        return Err("serve exited early".to_owned());
      }
      std::thread::sleep(Duration::from_millis(100));
    }
  }

  fn start() -> Result<Self, String> {
    let tmp = tempfile::TempDir::new().map_err(|err| format!("temp: {err}"))?;
    let resources = tmp.path().join("resources");
    let state = tmp.path().join("epic");
    for dir in [
      resources.join("spool"),
      tmp.path().join("home"),
      state.join("status"),
    ] {
      std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let mut rig = Self {
      tmp,
      resources,
      state,
      bin: toolu_binary()?,
      child: None,
    };
    rig.write_registry()?;
    rig.spawn_serve()?;
    rig.wait_live()?;
    Ok(rig)
  }

  fn toolu(&self, args: &[&str]) -> Result<Value, String> {
    let mut command = Command::new(&self.bin);
    command.arg("epic").args(args);
    self.env(&mut command);
    let output = command.output().map_err(|err| format!("toolu: {err}"))?;
    if !output.status.success() {
      return Err(format!(
        "toolu epic {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
      ));
    }
    serde_json::from_slice(&output.stdout).map_err(|err| format!("json stdout: {err}"))
  }

  fn journal_lines(&self) -> Vec<String> {
    let mut lines = Vec::new();
    let Ok(entries) = std::fs::read_dir(self.resources.join("journal")) else {
      return lines;
    };
    for entry in entries.flatten() {
      let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
      lines.extend(text.lines().map(str::to_owned));
    }
    lines
  }

  fn status_file(&self, key: &str) -> String {
    self
      .state
      .join("status")
      .join(format!("{key}.json"))
      .display()
      .to_string()
  }
}

impl Drop for Rig {
  fn drop(&mut self) {
    if let Some(mut child) = self.child.take() {
      let _killed = child.kill();
      let _waited = child.wait();
    }
  }
}

fn attention_has(status: &Value, kind: &str, key: &str) -> bool {
  status["attention"].as_array().is_some_and(|items| {
    items
      .iter()
      .any(|item| item["kind"] == kind && item["key"] == key)
  })
}

/// Poll the journal until every needle appears on one line, or time runs out.
fn wait_for_journal(rig: &Rig, needles: &[&str], secs: u64) -> bool {
  let deadline = Instant::now() + Duration::from_secs(secs);
  loop {
    let hit = rig
      .journal_lines()
      .iter()
      .any(|line| needles.iter().all(|needle| line.contains(needle)));
    if hit || Instant::now() >= deadline {
      return hit;
    }
    std::thread::sleep(Duration::from_millis(500));
  }
}

fn sibling_direction<'a>(status: &'a Value, key: &str) -> &'a Value {
  status["issues"]
    .as_array()
    .and_then(|issues| issues.iter().find(|issue| issue["key"] == key))
    .and_then(|issue| issue.get("direction"))
    .unwrap_or(&Value::Null)
}

#[test]
fn notify_reaches_attention_and_a_sibling_through_the_live_engine() {
  let rig = Rig::start().expect("rig");
  for key in ["a", "b"] {
    rig
      .toolu(&[
        "report",
        "execution",
        "--status-file",
        &rig.status_file(key),
      ])
      .expect("report");
  }
  // AC-1: the notice journals and raises attention.
  let noted = rig
    .toolu(&[
      "notify",
      "--status-file",
      &rig.status_file("a"),
      "--kind",
      "blocked",
      "--note",
      "waiting on b",
    ])
    .expect("notify");
  assert_eq!(noted["ok"], Value::Bool(true));
  // The overview document carries attention items and directions; the
  // per-epic detail only counts them.
  let status = rig.toolu(&["status"]).expect("status");
  assert!(attention_has(&status, "blocked", "a"), "{status}");
  // An unknown recipient is refused, not answered `ok`.
  let refused = rig.toolu(&[
    "notify",
    "--status-file",
    &rig.status_file("a"),
    "--kind",
    "help",
    "--to",
    "nobody",
  ]);
  assert!(refused.is_err(), "{refused:?}");
  // AC-6: an addressed notice is relayed to the sibling.
  relay_to_sibling(&rig).expect("relay");
}

/// AC-6: an addressed notice reaches the sibling through the live engine,
/// journaled and visible in status.
fn relay_to_sibling(rig: &Rig) -> Result<(), String> {
  let file = rig.status_file("a");
  let relayed = rig.toolu(&[
    "notify",
    "--status-file",
    &file,
    "--kind",
    "contract-changed",
    "--note",
    "API renamed",
    "--to",
    "b",
  ])?;
  if relayed.get("ok") != Some(&Value::Bool(true)) {
    return Err(format!("relay refused: {relayed}"));
  }
  let delivered = wait_for_journal(
    rig,
    &["\"kind\":\"prompt\"", "\"name\":\"relay\"", "API renamed"],
    30,
  );
  if !delivered {
    return Err("no journaled relay prompt for b".to_owned());
  }
  let status = rig.toolu(&["status"])?;
  let direction = sibling_direction(&status, "b");
  if direction.get("by") != Some(&Value::from("relay:a")) {
    return Err(format!("sibling direction missing: {status}"));
  }
  direction
    .get("text")
    .and_then(Value::as_str)
    .is_some_and(|text| text.contains("API renamed"))
    .then_some(())
    .ok_or_else(|| format!("direction text wrong: {status}"))
}
