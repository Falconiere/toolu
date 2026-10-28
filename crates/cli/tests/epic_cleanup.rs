//! Cleanup through the real binary, socket and dashboard, with preserved Git state.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

#[path = "helpers/epic.rs"]
mod epic;
use epic::{Home, Res, TOOLU, err};

struct Running(Child);
impl Drop for Running {
  fn drop(&mut self) {
    let _killed = self.0.kill();
    let _waited = self.0.wait();
  }
}

fn write(path: &Path, value: &Value) -> Res<()> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  Ok(std::fs::write(path, serde_json::to_vec(value)?)?)
}

fn seed(home: &Home) -> Res<()> {
  let dir = home.path("done");
  write(
    &dir.join("graph.json"),
    &json!({
      "epic": {"ref": "o/r#1", "title": "done", "state": "closed", "url": ""},
      "state_dir": dir, "issues": [], "complete": true
    }),
  )?;
  write(
    &home.path("resources/registry.json"),
    &json!({"version": 1, "extra": "keep",
    "epics": [{"key": "done", "state_dir": dir}]}),
  )
}

fn cleanup(home: &Home, dry: bool) -> Res<Value> {
  let args = if dry {
    vec!["epic", "cleanup", "--dry-run"]
  } else {
    vec!["epic", "cleanup"]
  };
  let output = home.run(&args)?;
  if !output.status.success() {
    return Err(err(&output).into());
  }
  Ok(serde_json::from_slice(&output.stdout)?)
}

fn start(home: &Home) -> Res<Running> {
  let mut command = Command::new(TOOLU);
  command
    .env_clear()
    .env("PATH", "/usr/bin:/bin")
    .env("HOME", home.path("home"))
    .env("TOOLU_RESOURCE_HOME", home.path("resources"))
    .env("TOOLU_EPIC_HERDR", "off")
    .args(["epic", "engine", "--http", "127.0.0.1:0"])
    .stdout(Stdio::null())
    .stderr(Stdio::piped());
  if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let running = Running(command.spawn()?);
  let deadline = Instant::now() + Duration::from_secs(10);
  while !home.path("resources/http.json").exists() {
    if Instant::now() >= deadline {
      return Err("engine did not bind HTTP".into());
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  Ok(running)
}

fn socket(home: &Home, request: &Value) -> Res<Value> {
  let mut stream = UnixStream::connect(home.path("resources/engine.sock"))?;
  stream.set_read_timeout(Some(Duration::from_secs(5)))?;
  let mut reader = BufReader::new(stream.try_clone()?);
  let mut line = String::new();
  reader.read_line(&mut line)?;
  stream.write_all(format!("{request}\n").as_bytes())?;
  line.clear();
  reader.read_line(&mut line)?;
  Ok(serde_json::from_str(&line)?)
}

fn dashboard(home: &Home) -> Res<Value> {
  let info: Value = serde_json::from_slice(&std::fs::read(home.path("resources/http.json"))?)?;
  let addr = info
    .get("addr")
    .and_then(Value::as_str)
    .ok_or("no address")?;
  let mut stream = TcpStream::connect(addr)?;
  stream.set_read_timeout(Some(Duration::from_secs(5)))?;
  stream.write_all(
    format!("GET /api/v1/overview HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
      .as_bytes(),
  )?;
  let mut response = String::new();
  stream.read_to_string(&mut response)?;
  if !response.starts_with("HTTP/1.1 200 ") {
    return Err(response.into());
  }
  Ok(serde_json::from_str(
    response.split_once("\r\n\r\n").ok_or("no body")?.1,
  )?)
}

fn git_worktree(home: &Home) -> Res<std::path::PathBuf> {
  let repo = home.path("repo");
  std::fs::create_dir_all(&repo)?;
  for args in [
    vec!["init", "-q"],
    vec![
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.com",
      "commit",
      "--allow-empty",
      "-qm",
      "init",
    ],
    vec!["worktree", "add", "-qb", "preserved", "../worker"],
  ] {
    if !Command::new("git")
      .current_dir(&repo)
      .args(args)
      .status()?
      .success()
    {
      return Err("Git worktree setup failed".into());
    }
  }
  Ok(repo)
}

#[test]
fn live_cleanup_disappears_from_socket_and_dashboard_and_stays_absent_after_restart() {
  let home = Home::new().unwrap();
  seed(&home).unwrap();
  let mut running = start(&home).unwrap();
  assert_eq!(
    dashboard(&home).unwrap()["epics"].as_array().unwrap().len(),
    1
  );
  let before = std::fs::read(home.path("resources/registry.json")).unwrap();
  assert_eq!(
    cleanup(&home, true).unwrap()["removed"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
  assert_eq!(
    std::fs::read(home.path("resources/registry.json")).unwrap(),
    before
  );
  assert_eq!(
    socket(&home, &json!({"op": "status"})).unwrap()["epics"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
  assert_eq!(
    cleanup(&home, false).unwrap()["removed"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
  assert_eq!(
    socket(&home, &json!({"op": "status"})).unwrap()["epics"],
    json!([])
  );
  assert_eq!(dashboard(&home).unwrap()["epics"], json!([]));
  assert!(home.path("done/graph.json").is_file());
  assert_eq!(socket(&home, &json!({"op": "stop"})).unwrap()["ok"], true);
  assert!(running.0.wait().unwrap().success());
  let _restarted = start(&home).unwrap();
  assert_eq!(dashboard(&home).unwrap()["epics"], json!([]));
  assert_eq!(cleanup(&home, false).unwrap()["removed"], json!([]));
}

#[test]
fn offline_cleanup_preserves_git_worktree_branch_and_journal() {
  let home = Home::new().unwrap();
  seed(&home).unwrap();
  let repo = git_worktree(&home).unwrap();
  let journal = home.path("resources/journal/2026-01-01.jsonl");
  std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
  std::fs::write(
    &journal,
    "{\"seq\":1,\"kind\":\"action\",\"name\":\"epic-closed\",\"key\":\"done\"}\n",
  )
  .unwrap();
  let original = std::fs::read(&journal).unwrap();
  assert_eq!(
    cleanup(&home, false).unwrap()["removed"]
      .as_array()
      .unwrap()
      .len(),
    1
  );
  assert_eq!(std::fs::read(&journal).unwrap(), original);
  assert!(home.path("worker/.git").is_file());
  assert!(
    Command::new("git")
      .current_dir(&repo)
      .args(["show-ref", "--verify", "--quiet", "refs/heads/preserved"])
      .status()
      .unwrap()
      .success()
  );
  assert!(!home.path("resources/engine.sock").exists());
  assert!(!home.path("resources/engine.lock").exists());
}

#[test]
fn a_live_owner_without_a_socket_never_falls_back_to_offline_writes() {
  let home = Home::new().unwrap();
  seed(&home).unwrap();
  std::fs::write(
    home.path("resources/engine.lock"),
    format!("{}\n", std::process::id()),
  )
  .unwrap();
  let before = std::fs::read(home.path("resources/registry.json")).unwrap();
  let output = home.run(&["epic", "cleanup"]).unwrap();
  assert!(!output.status.success());
  assert_eq!(
    std::fs::read(home.path("resources/registry.json")).unwrap(),
    before
  );
  assert!(home.path("resources/engine.lock").is_file());
}
