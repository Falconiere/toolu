use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{close, gate};
use crate::support::matches;

#[test]
fn an_incomplete_epic_is_refused_and_a_complete_one_dry_runs_its_summary() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut graph = crate::support::graph_248();
  graph.state_dir = tmp.path().display().to_string();
  let file = tmp.path().join("graph.json");
  crate::graph::write_graph(&file, &graph).expect("graph");
  let file = file.display().to_string();
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let refused = close(
    &matches(&["epic", "close", "--graph", &file, "--dry-run"]),
    &Ctx::default(),
    &env,
  );
  assert_eq!(refused.exit, Exit::Failure);
  graph.complete = true;
  crate::graph::write_graph(tmp.path().join("graph.json").as_path(), &graph).expect("graph");
  let summary = close(
    &matches(&["epic", "close", "--graph", &file, "--dry-run"]),
    &Ctx::default(),
    &env,
  );
  assert!(
    summary
      .stdout
      .is_some_and(|text| text.starts_with("Epic complete."))
  );
}

#[test]
fn the_gate_without_github_fails_before_any_change() {
  let env = Env::from_pairs([("HOME", "/nonexistent"), ("PATH", "")]);
  let outcome = gate(&matches(&["epic", "gate", "--pr", "o/r#7"]), &env);
  assert_eq!(outcome.exit, Exit::Failure);
}

/// A stand-in engine on `paths` that answers one request and returns it.
fn stand_in(paths: &crate::paths::Paths) -> std::thread::JoinHandle<serde_json::Value> {
  use std::io::{BufRead, BufReader, Write};
  std::fs::write(paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  let listener = std::os::unix::net::UnixListener::bind(paths.socket()).expect("bind");
  listener.set_nonblocking(true).expect("nonblocking");
  std::thread::spawn(move || {
    // Give up after 5 s, so a gate that never connects fails the test
    // instead of hanging it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut stream = loop {
      match listener.accept() {
        Ok((stream, _)) => break stream,
        Err(_) if std::time::Instant::now() < deadline => {
          std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Err(err) => return serde_json::json!({"error": err.to_string()}),
      }
    };
    stream.set_nonblocking(false).expect("blocking");
    let hello = serde_json::json!({"op": "hello", "protocol": crate::PROTOCOL});
    writeln!(stream, "{hello}").expect("hello");
    let mut line = String::new();
    BufReader::new(stream.try_clone().expect("clone"))
      .read_line(&mut line)
      .expect("request");
    writeln!(stream, "{}", serde_json::json!({"ok": true})).expect("ack");
    serde_json::from_str(&line).expect("json")
  })
}

#[test]
fn a_merged_gate_tells_a_live_engine_and_a_dry_run_or_a_keyless_one_does_not() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([("TOOLU_RESOURCE_HOME", tmp.path().display().to_string())]);
  let paths = crate::paths::Paths::from_env(&env);
  let merged = serde_json::json!({"merged": true});
  let opts = |key: Option<&str>, dry_run: bool| crate::gate::GateOpts {
    pr: "o/r#7".into(),
    key: key.map(str::to_owned),
    dry_run,
    ..crate::gate::GateOpts::default()
  };
  let engine = stand_in(&paths);
  // The stand-in answers only the first request: a dry run, a gate without
  // `--key` or an unmerged PR would be the one it records.
  super::notify_merged(&opts(Some("dry"), true), &merged, &env);
  super::notify_merged(&opts(None, false), &merged, &env);
  super::notify_merged(&opts(Some("open"), false), &serde_json::json!({}), &env);
  super::notify_merged(&opts(Some("r-5"), false), &merged, &env);
  let sent = engine.join().expect("engine");
  assert_eq!(
    (
      sent["op"].clone(),
      sent["key"].clone(),
      sent["event"].clone()
    ),
    (
      serde_json::json!("event"),
      serde_json::json!("r-5"),
      serde_json::json!("merged")
    )
  );
}
