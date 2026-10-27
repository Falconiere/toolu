//! The `notify` verb against a closed engine: rejection before any write
//! and spooling for later ingest.

use clap::{Arg, Command};

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::notify;
use super::tests::{parse, root};
use crate::paths::Paths;

fn closed_engine(path: &std::path::Path) -> (Paths, Env) {
  let (paths, env) = root(path);
  crate::disk::write_value(&paths.registry(), &json!({"version": 1, "epics": []}))
    .expect("registry");
  std::fs::create_dir_all(paths.root.join("spool")).expect("spool");
  (paths, env)
}

#[test]
fn notify_rejects_unknown_kinds_before_any_write() {
  assert!(
    crate::verbs::command()
      .try_get_matches_from([
        "epic",
        "notify",
        "--status-file",
        "s/status/a.json",
        "--kind",
        "frobnicate",
      ])
      .is_err()
  );
}

#[test]
fn notify_spools_without_a_live_engine() {
  let tmp = tempfile::tempdir().expect("temp");
  let (paths, env) = closed_engine(tmp.path());
  let file = tmp.path().join("s").join("status").join("a.json");
  let missing = notify(
    &Command::new("notify")
      .arg(Arg::new("kind").long("kind"))
      .arg(Arg::new("status-file").long("status-file"))
      .get_matches_from(["notify", "--kind", "blocked"]),
    &env,
  );
  assert!(
    missing
      .stderr
      .expect("err")
      .contains("status-file is required")
  );
  let outcome = notify(
    &parse(&[
      "epic",
      "notify",
      "--status-file",
      &file.display().to_string(),
      "--kind",
      "blocked",
      "--note",
      "waiting on #12",
    ]),
    &env,
  );
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  let body = outcome.stdout.expect("stdout");
  assert!(body.contains("\"ok\":true"), "{body}");
  let spooled: Vec<_> = std::fs::read_dir(paths.root.join("spool"))
    .expect("spool")
    .collect();
  assert_eq!(spooled.len(), 1);
  let raw = std::fs::read_to_string(spooled[0].as_ref().expect("entry").path()).expect("read");
  let request: serde_json::Value = serde_json::from_str(&raw).expect("json");
  assert_eq!(request["op"], json!("notify"));
  assert_eq!(request["key"], json!("a"));
  assert_eq!(request["kind"], json!("blocked"));
  assert_eq!(request["note"], json!("waiting on #12"));
}
