//! Subscription-test fixture: a temp epic and a scripted herdr peer.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::paths::Paths;

const CLOSED: &str =
  r#"{"event":"pane_closed","data":{"type":"pane_closed","pane_id":"w1:p1","workspace_id":"w1"}}"#;

pub(crate) struct Fix {
  pub(crate) tmp: tempfile::TempDir,
  pub(crate) engine_paths: Paths,
  pub(crate) env: Env,
  pub(crate) listener: UnixListener,
  pub(crate) script: Script,
}

#[derive(Clone)]
pub(crate) struct Script {
  pub(crate) agents: Value,
  pub(crate) event: Option<String>,
  pub(crate) read: Option<String>,
  pub(crate) drop_after_subscribe: bool,
  pub(crate) empty_after_first: bool,
  pub(crate) log: Option<std::path::PathBuf>,
}

fn write_epic(epic: &std::path::Path) -> Result<(), String> {
  std::fs::create_dir_all(epic.join("issues")).map_err(|err| err.to_string())?;
  std::fs::create_dir_all(epic.join("status")).map_err(|err| err.to_string())?;
  std::fs::write(
    epic.join("graph.json"),
    r#"{"epic":"one","issues":[{"key":"a","open_blockers":[]}]}"#,
  )
  .map_err(|err| err.to_string())?;
  std::fs::write(
    epic.join("issues").join("a.json"),
    r#"{"stage":"running","worktree":"/work","kind":"claude"}"#,
  )
  .map_err(|err| err.to_string())?;
  std::fs::write(
    epic.join("status").join("a.json"),
    r#"{"phase":"execution"}"#,
  )
  .map_err(|err| err.to_string())?;
  Ok(())
}

pub(crate) fn fixture(with_agent: bool) -> Result<Fix, String> {
  let tmp = tempfile::tempdir().map_err(|err| err.to_string())?;
  let epic = tmp.path().join("epic");
  write_epic(&epic)?;
  let paths = Paths::at(tmp.path());
  crate::disk::write_value(
    &paths.registry(),
    &json!({"version": 1, "epics": [{"key": "one", "state_dir": epic.display().to_string()}]}),
  )?;
  let sock = tmp.path().join("herdr.sock");
  let listener = UnixListener::bind(&sock).map_err(|err| err.to_string())?;
  let agents = if with_agent {
    json!([{"name": "a", "pane_id": "w1:p1", "cwd": "/work", "agent_status": "idle"}])
  } else {
    json!([])
  };
  Ok(Fix {
    tmp,
    engine_paths: paths,
    env: Env::from_pairs([
      ("HOME", "/tmp".to_owned()),
      ("HERDR_SOCKET_PATH", sock.display().to_string()),
      ("PATH", "/usr/bin:/bin".to_owned()),
    ]),
    listener,
    script: Script {
      agents,
      event: Some(CLOSED.to_owned()),
      read: None,
      drop_after_subscribe: false,
      empty_after_first: false,
      log: None,
    },
  })
}

pub(crate) fn serve_peer(listener: UnixListener, script: Script) -> thread::JoinHandle<()> {
  thread::spawn(move || {
    let seen = std::sync::atomic::AtomicUsize::new(0);
    for _ in 0..4 {
      let Ok((stream, _)) = listener.accept() else {
        break;
      };
      let n = seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      let mut script = script.clone();
      if script.empty_after_first && n > 0 {
        script.agents = json!([]);
      }
      thread::spawn(move || handle(stream, &script));
    }
  })
}

fn handle(mut stream: UnixStream, script: &Script) {
  let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
  while let Ok(line) = read_line(&mut stream) {
    let Ok(req) = serde_json::from_str::<Value>(&line) else {
      break;
    };
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    if let Some(log) = &script.log {
      let _saved = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut file| writeln!(file, "{method}"));
    }
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    if method == "events.subscribe" {
      let _ = write_line(
        &mut stream,
        &json!({"id": id, "result": {"type": "subscription_started"}}),
      );
      if let Some(event) = &script.event {
        let _ = stream.write_all(event.as_bytes());
        let _ = stream.write_all(b"\n");
      }
      if script.drop_after_subscribe {
        break;
      }
      continue;
    }
    let result = if method == "ping" {
      json!({"type": "pong", "protocol": 22})
    } else if method == "session.snapshot" {
      json!({"type": "session_snapshot", "snapshot": {"agents": script.agents}})
    } else if method == "agent.read" {
      json!({"stdout": script.read.clone().unwrap_or_default()})
    } else {
      json!({})
    };
    if write_line(&mut stream, &json!({"id": id, "result": result})).is_err() {
      break;
    }
  }
}

fn read_line(stream: &mut UnixStream) -> Result<String, ()> {
  let mut buf = Vec::new();
  let mut byte = [0_u8; 1];
  loop {
    let n = stream.read(&mut byte).map_err(|_err| ())?;
    if n == 0 || byte[0] == b'\n' {
      break;
    }
    buf.push(byte[0]);
  }
  String::from_utf8(buf).map_err(|_err| ())
}

fn write_line(stream: &mut UnixStream, value: &Value) -> Result<(), ()> {
  let mut line = serde_json::to_string(value).map_err(|_err| ())?;
  line.push('\n');
  stream.write_all(line.as_bytes()).map_err(|_err| ())
}

#[cfg(test)]
#[path = "tests/source_fix_test.rs"]
mod tests;
