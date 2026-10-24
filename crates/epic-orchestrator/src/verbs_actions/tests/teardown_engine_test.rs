//! `toolu epic finish` against an engine served on a real Unix socket (#619).

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::finish;
use crate::client::exchange_retry;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::{ServeOptions, serve_with_http};
use crate::support::matches;

struct Setup {
  _tmp: tempfile::TempDir,
  env: Env,
  paths: Paths,
  state: PathBuf,
  record: PathBuf,
}

/// A registered epic `one` whose issue `a` is `running` but never made a
/// worktree, agent or pane, so its abandon needs no herdr.
fn setup() -> Setup {
  let tmp = tempfile::tempdir().expect("temp");
  let root = std::fs::canonicalize(tmp.path()).expect("real");
  let (resources, state) = (root.join("r"), root.join("epic"));
  let record = state.join("issues/a.json");
  let write = |path: PathBuf, body: Value| crate::disk::write_value(&path, &body).expect("write");
  write(
    state.join("graph.json"),
    json!({"epic": "one", "issues": [{"key": "a", "open_blockers": []}]}),
  );
  write(
    record.clone(),
    json!({"key": "a", "repo": "o/r", "branch": "feat/a", "stage": "running"}),
  );
  write(state.join("status/a.json"), json!({"phase": "execution"}));
  write(
    resources.join("registry.json"),
    json!({"version": 1, "epics": [{"key": "one", "state_dir": state}]}),
  );
  let env = Env::from_pairs([
    ("HOME", root.display().to_string()),
    ("TOOLU_RESOURCE_HOME", resources.display().to_string()),
  ]);
  Setup {
    _tmp: tmp,
    paths: Paths::at(&resources),
    env,
    state,
    record,
  }
}

fn wait_live(paths: &Paths) {
  for _ in 0..500 {
    if crate::lock::live(&paths.lock()) && paths.socket().exists() {
      return;
    }
    thread::sleep(Duration::from_millis(20));
  }
  panic!("engine never came up");
}

fn finish_line(s: &Setup) -> toolu_runtime::cli::Outcome {
  let state = s.state.display().to_string();
  finish(
    &matches(&["epic", "finish", &state, "a", "--abandon"]),
    &s.env,
  )
}

/// The `stage` of the first object in `doc` whose `key` is `key`.
fn stage_of<'a>(doc: &'a Value, key: &str) -> Option<&'a str> {
  match doc {
    Value::Object(map) if map.get("key").and_then(Value::as_str) == Some(key) => {
      map.get("stage").and_then(Value::as_str)
    }
    Value::Object(map) => map.values().find_map(|value| stage_of(value, key)),
    Value::Array(items) => items.iter().find_map(|value| stage_of(value, key)),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
  }
}

#[test]
fn an_abandon_through_a_served_engine_settles_it_without_a_relaunch() {
  let s = setup();
  let served = s.paths.clone();
  let engine = thread::spawn(move || {
    serve_with_http(
      served,
      None,
      Fault::None,
      crate::PROTOCOL,
      ServeOptions {
        http: None,
        tick: &(),
        attention: crate::notify::AttentionOptions::default(),
      },
    )
  });
  wait_live(&s.paths);
  let outcome = finish_line(&s);
  assert_eq!(outcome.exit, Exit::Success, "{:?}", outcome.stderr);
  assert_eq!(
    crate::disk::read_value(&s.record).expect("record")["stage"],
    "abandoned"
  );
  let status = exchange_retry(
    &s.paths,
    crate::PROTOCOL,
    &json!({"op": "status", "epic": "one"}),
  )
  .expect("status");
  assert_eq!(stage_of(&status, "a"), Some("abandoned"), "{status}");
  let stop = exchange_retry(&s.paths, crate::PROTOCOL, &json!({"op": "stop"})).expect("stop");
  assert_eq!(stop["ok"], true);
  engine.join().expect("engine").expect("serve");
  let rows = crate::journal::tail(&s.paths.journal_dir(), SystemTime::now()).expect("journal");
  let notes: Vec<&str> = rows
    .iter()
    .filter(|row| row.name == "finish" && row.key == "a")
    .map(|row| row.note.as_str())
    .collect();
  assert_eq!(notes, ["begin", "abandoned"]);
  assert!(
    !rows
      .iter()
      .any(|row| row.name.ends_with("-intent") && row.key == "a")
  );
}

#[test]
fn an_engine_that_cannot_take_the_finish_leaves_the_record_untouched() {
  let s = setup();
  std::fs::write(s.paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  let listener = UnixListener::bind(s.paths.socket()).expect("bind");
  let old = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept");
    stream.write_all(b"{\"protocol\":2}\n").expect("hello");
    let mut line = String::new();
    BufReader::new(stream.try_clone().expect("clone"))
      .read_line(&mut line)
      .expect("request");
    stream
      .write_all(b"{\"error\":\"unknown op\"}\n")
      .expect("reply");
    line
  });
  let before = std::fs::read(&s.record).expect("bytes");
  let outcome = finish_line(&s);
  let request: Value = serde_json::from_str(&old.join().expect("old engine")).expect("json");
  assert_eq!(
    (request["op"].clone(), request["step"].clone()),
    (json!("finish"), json!("begin"))
  );
  assert_eq!(outcome.exit, Exit::Failure);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some(
      "toolu epic finish: the running engine cannot take a finish; replace it with `toolu epic engine --replace`"
    )
  );
  assert_eq!(std::fs::read(&s.record).expect("bytes"), before);
}
