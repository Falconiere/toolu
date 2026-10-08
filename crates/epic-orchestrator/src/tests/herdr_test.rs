use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::{Call, Client};
use crate::client::{read_json, write_json};
use crate::journal;
use crate::paths::Paths;

#[test]
fn herdr_protocol_fallback() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let sock = tmp.path().join("herdr.sock");
  let log = tmp.path().join("argv.log");
  let bin = script_dir(tmp.path(), &log);
  let listener = UnixListener::bind(&sock).expect("bind");
  let sink = PathBuf::from("/dev/null");
  let peer = thread::spawn(move || answer(&listener, 99, 1, &sink));
  let mut client = Client::new();
  client
    .execute(&paths, &herdr_env(&sock, &bin, None), &prompt("STATUS?"))
    .expect("cli");
  client
    .execute(
      &paths,
      &herdr_env(&sock, &bin, Some("toolu446")),
      &prompt("again"),
    )
    .expect("second");
  peer.join().expect("peer");
  let rows = journal::tail(&paths.journal_dir(), SystemTime::now()).expect("journal");
  let warnings: Vec<_> = rows
    .iter()
    .filter(|row| row.name == "herdr-protocol")
    .collect();
  assert_eq!(warnings.len(), 1);
  assert_eq!(warnings[0].note, "protocol 99");
  let lines = std::fs::read_to_string(&log).expect("log");
  let mut recorded = lines.lines();
  let first = recorded.next().unwrap_or("");
  assert!(first.starts_with("herdr agent prompt"), "{lines:?}");
  assert!(recorded.next().unwrap_or("").contains("--session toolu446"));
}

#[test]
fn herdr_control() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let sock = tmp.path().join("herdr.sock");
  let log = tmp.path().join("argv.log");
  let bin = script_dir(tmp.path(), &log);
  let methods = tmp.path().join("methods.log");
  let listener = UnixListener::bind(&sock).expect("bind");
  let record = methods.clone();
  let peer = thread::spawn(move || answer(&listener, 22, 5, &record));
  let env = herdr_env(&sock, &bin, None);
  let mut client = Client::new();
  for call in calls() {
    client.execute(&paths, &env, &call).expect("socket");
  }
  peer.join().expect("peer");
  let seen = std::fs::read_to_string(&methods).expect("methods");
  for method in [
    "agent.start",
    "agent.prompt",
    "agent.read",
    "worktree.create",
    "worktree.remove",
  ] {
    assert!(seen.contains(method), "{seen}");
  }
  assert_eq!(std::fs::read_to_string(&log).unwrap_or_default(), "");
}

fn calls() -> [Call; 5] {
  crate::herdr_argv::sample_calls()
}

fn prompt(text: &str) -> Call {
  Call::Prompt {
    target: "a".into(),
    text: text.to_owned(),
  }
}

fn herdr_env(sock: &Path, bin: &Path, session: Option<&str>) -> Env {
  let mut env = Env::from_pairs([
    ("PATH", format!("{}:/usr/bin:/bin", bin.display())),
    ("HOME", "/tmp".to_owned()),
    ("HERDR_SOCKET_PATH", sock.display().to_string()),
  ]);
  if let Some(session) = session {
    env = env.with("TOOLU_EPIC_HERDR_SESSION", session);
  }
  env
}

fn script_dir(root: &Path, log: &Path) -> PathBuf {
  let bin = root.join("bin");
  std::fs::create_dir_all(&bin).expect("bin");
  let path = bin.join("herdr");
  let mut file = std::fs::File::create(&path).expect("script");
  writeln!(
    file,
    "#!/bin/sh\nbase=$(basename \"$0\")\nprintf '%s\\n' \"$base $*\" >> '{}'",
    log.display()
  )
  .expect("write");
  let mut perms = std::fs::metadata(&path).expect("meta").permissions();
  perms.set_mode(0o755);
  std::fs::set_permissions(&path, perms).expect("chmod");
  bin
}

fn answer(listener: &UnixListener, protocol: u64, accepts: usize, methods: &Path) {
  for _ in 0..accepts {
    let Ok((mut stream, _)) = listener.accept() else {
      break;
    };
    while let Ok(req) = read_json(&mut stream) {
      let method = req.get("method").and_then(Value::as_str).unwrap_or("");
      let id = req.get("id").cloned().unwrap_or(Value::Null);
      let result = if method == "ping" {
        json!({"type": "pong", "protocol": protocol})
      } else {
        json!({})
      };
      if write_json(&mut stream, &json!({"id": id, "result": result})).is_err() {
        break;
      }
      if method != "ping" {
        let _saved = std::fs::OpenOptions::new()
          .create(true)
          .append(true)
          .open(methods)
          .and_then(|mut file| writeln!(file, "{method}"));
        break;
      }
    }
  }
}
