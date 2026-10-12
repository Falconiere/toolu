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
  let peer = thread::spawn(move || answer(&listener, 22, 6, &record));
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
    "notification.show",
  ] {
    assert!(seen.contains(method), "{seen}");
  }
  assert_eq!(std::fs::read_to_string(&log).unwrap_or_default(), "");
}

fn calls() -> [Call; 6] {
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

/// A linked worktree herdr created for `feat/1-x`: its path, workspace and pane.
fn created(call: &mut dyn FnMut(Call) -> Value, repo: &Path) -> (PathBuf, String, String) {
  let created = call(Call::Create {
    cwd: Some(repo.display().to_string()),
    branch: Some("feat/1-x".into()),
    base: Some("main".into()),
    path: None,
    label: Some("r-1".into()),
    workspace: None,
  });
  let text = |pointer: &str| {
    created
      .pointer(pointer)
      .and_then(Value::as_str)
      .expect("field")
      .to_owned()
  };
  (
    PathBuf::from(text("/worktree/path")),
    text("/workspace/workspace_id"),
    text("/root_pane/pane_id"),
  )
}

/// `worktree.list` shows `feat/1-x` open in `workspace`.
fn assert_open(call: &mut dyn FnMut(Call) -> Value, repo: &Path, workspace: &str) {
  let listed = call(Call::WorktreeList {
    cwd: repo.display().to_string(),
  });
  let rows = listed["worktrees"].as_array().cloned().unwrap_or_default();
  let row = rows.iter().find(|row| row["branch"] == "feat/1-x");
  assert_eq!(
    row.map(|row| row["open_workspace_id"].clone()),
    Some(json!(workspace))
  );
}

#[test]
fn the_socket_fixture_runs_a_worktree_and_agent_lifecycle() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let repo = tmp.path().join("repo");
  crate::support::repo_with_origin(&repo, "https://github.com/o/r.git");
  let herdr = crate::herdr_fixture::HerdrFixture::start();
  herdr.add_repo(&repo);
  let env = herdr.env();
  let mut client = Client::new();
  let mut call =
    |call: Call| crate::herdr_calls::request(&mut client, &paths, &env, &call).expect("call");
  let (worktree, workspace, pane) = created(&mut call, &repo);
  assert!(worktree.join(".git").is_file());
  assert_open(&mut call, &repo, &workspace);
  let start = Call::Start {
    name: "r-1".into(),
    kind: "claude".into(),
    pane_id: pane.clone(),
    args: vec![],
    timeout_ms: None,
  };
  call(start);
  assert_eq!(
    call(Call::AgentList)["agents"][0]["cwd"],
    json!(worktree.display().to_string())
  );
  let info = call(Call::PaneInfo { pane_id: pane });
  let shell =
    u32::try_from(info["process_info"]["shell_pid"].as_u64().expect("shell")).expect("pid");
  let owned = crate::workload::owned_workload(&worktree, Some(shell)).expect("owned");
  assert_eq!(owned.len(), 1, "{owned:?}");
  call(Call::Prompt {
    target: "r-1".into(),
    text: "/exit".into(),
  });
  assert_eq!(call(Call::AgentList)["agents"], json!([]));
  call(Call::Remove {
    workspace_id: workspace,
    force: true,
  });
  assert!(!worktree.exists());
  let methods: Vec<String> = herdr
    .calls()
    .into_iter()
    .map(|(method, _)| method)
    .collect();
  assert_eq!(methods.first().map(String::as_str), Some("worktree.create"));
}

#[test]
fn a_socket_read_past_its_deadline_reads_as_timed_out() {
  let tmp = tempfile::tempdir().expect("temp");
  let sock = tmp.path().join("silent.sock");
  let listener = UnixListener::bind(&sock).expect("bind");
  let peer = thread::spawn(move || listener.accept().map(|(stream, _)| stream));
  let mut stream = std::os::unix::net::UnixStream::connect(&sock).expect("connect");
  stream
    .set_read_timeout(Some(std::time::Duration::from_millis(50)))
    .expect("deadline");
  assert_eq!(
    crate::client::read_text(&mut stream),
    Err("timed out".to_owned())
  );
  drop(peer.join());
}
