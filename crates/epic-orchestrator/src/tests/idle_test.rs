use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::json;

use crate::TICK;

#[test]
fn idle_tick() {
  assert!(TICK.as_secs() >= 30);
  let binary = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/release/toolu");
  if !binary.is_file() {
    return;
  }
  let rss = rss_kib(&binary);
  assert!(rss > 0 && rss <= 10_240, "{rss}");
  let evidence =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/toolu/evidence/epic-engine-idle.json");
  if let Some(parent) = evidence.parent() {
    std::fs::create_dir_all(parent).expect("evidence dir");
  }
  let doc = json!({"epics": 3, "rss_kib": rss, "tick_secs": TICK.as_secs()});
  std::fs::write(&evidence, format!("{doc}\n")).expect("evidence");
}

fn rss_kib(binary: &Path) -> u64 {
  let tmp = tempfile::tempdir().expect("temp");
  let mut epics = Vec::new();
  for name in ["one", "two", "three"] {
    let epic = tmp.path().join(name);
    std::fs::create_dir_all(epic.join("issues")).expect("epic");
    std::fs::write(epic.join("graph.json"), "{\"issues\":[]}").expect("graph");
    epics.push(json!({"key": name, "state_dir": epic.display().to_string()}));
  }
  let registry = json!({"version": 1, "epics": epics});
  std::fs::write(tmp.path().join("registry.json"), format!("{registry}\n")).expect("registry");
  let mut child = Command::new(binary)
    .args(["epic", "engine"])
    .env_clear()
    .env("HOME", tmp.path())
    .env("TOOLU_RESOURCE_HOME", tmp.path())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("engine");
  thread::sleep(Duration::from_secs(2));
  let status = std::fs::read_to_string(format!("/proc/{}/status", child.id())).unwrap_or_default();
  let _killed = child.kill();
  let _waited = child.wait();
  vm_rss(&status)
}

fn vm_rss(status: &str) -> u64 {
  status
    .lines()
    .find(|line| line.starts_with("VmRSS:"))
    .and_then(|line| line.split_whitespace().nth(1))
    .and_then(|kb| kb.parse().ok())
    .unwrap_or(0)
}
