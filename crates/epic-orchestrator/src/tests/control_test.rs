use std::path::Path;

use clap::{Arg, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{
  attention_options, engine, ensure, epic_key, fault_of, http_config, register, registry_nonempty,
  start,
};
use crate::disk::read_value;
use crate::paths::Paths;
use crate::server::Fault;

#[test]
fn a_directory_name_is_the_epic_key_without_a_graph() {
  let key = epic_key(Path::new("/epics/falconiere-toolu-402"));
  assert_eq!(key, "falconiere-toolu-402");
}

#[test]
fn a_path_without_a_file_name_uses_the_full_path() {
  let path = Path::new("/no-such-epic-434/..");
  assert!(path.file_name().is_none());
  assert_eq!(epic_key(path), "/no-such-epic-434/..");
}

#[test]
fn a_graph_epic_names_the_key() {
  let tmp = tempfile::tempdir().expect("temp");
  let dir = tmp.path().join("falconiere-toolu-402");
  std::fs::create_dir_all(&dir).expect("dir");
  std::fs::write(dir.join("graph.json"), "{\"epic\":\"from-graph\"}\n").expect("graph");
  assert_eq!(epic_key(&dir), "from-graph");
  std::fs::write(dir.join("graph.json"), "{\"epic\":\"\"}\n").expect("empty");
  assert_eq!(epic_key(&dir), "falconiere-toolu-402");
}

#[test]
fn register_is_idempotent() {
  let tmp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(tmp.path());
  let dir = tmp.path().join("epic");
  std::fs::create_dir_all(&dir).expect("dir");
  assert!(!registry_nonempty(&paths));
  register(&paths, &dir.display().to_string()).expect("register");
  register(&paths, &dir.display().to_string()).expect("again");
  let rows = read_value(&paths.registry()).expect("registry");
  assert_eq!(rows["epics"].as_array().expect("epics").len(), 1);
  assert!(registry_nonempty(&paths));
  crate::disk::write_value(&paths.registry(), &serde_json::json!({"epics": []})).expect("empty");
  assert!(!registry_nonempty(&paths));
}

#[test]
fn fault_names_select_the_injected_fault() {
  assert_eq!(fault_of(&Env::default()), Fault::None);
  assert_eq!(
    fault_of(&Env::from_pairs([("TOOLU_EPIC_FAULT", "before-merge")])),
    Fault::BeforeMerge
  );
  assert_eq!(
    fault_of(&Env::from_pairs([("TOOLU_EPIC_FAULT", "after-merge")])),
    Fault::AfterMerge
  );
  assert_eq!(
    fault_of(&Env::from_pairs([(
      "TOOLU_EPIC_FAULT",
      "before-merge-record"
    )])),
    Fault::BeforeMergeRecord
  );
  assert_eq!(
    fault_of(&Env::from_pairs([("TOOLU_EPIC_FAULT", "other")])),
    Fault::None
  );
}

#[test]
fn ensure_does_nothing_for_an_empty_or_live_root() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env_at(tmp.path());
  ensure(&env).expect("empty");
  let paths = Paths::from_env(&env);
  crate::disk::write_value(
    &paths.registry(),
    &serde_json::json!({"epics": [{"key": "one", "state_dir": "/epic"}]}),
  )
  .expect("registry");
  std::fs::write(paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  ensure(&env).expect("live");
}

#[test]
fn a_held_lock_makes_the_foreground_engine_busy() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env_at(tmp.path());
  let paths = Paths::from_env(&env);
  std::fs::write(paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  let outcome = engine(&verb(&["epic", "engine"]), &env, &());
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(outcome.stderr.expect("err").contains("engine-busy"));
}

#[test]
fn engine_http_defaults_on_and_no_http_turns_it_off() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([
    ("HOME", tmp.path().display().to_string()),
    (
      "TOOLU_CONFIG_DIR",
      tmp.path().join("config").display().to_string(),
    ),
  ]);
  let http = http_config(&env, &verb(&["epic", "engine"]))
    .expect("default HTTP config")
    .expect("listener enabled by default");
  assert_eq!(http.addr.to_string(), "127.0.0.1:7717");
  assert!(
    http_config(&env, &verb(&["epic", "engine", "--no-http"]))
      .expect("disabled HTTP config")
      .is_none()
  );
}

#[test]
fn a_replace_that_cannot_drain_reports_it_and_ensure_stays_idle() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env_at(tmp.path());
  let paths = Paths::from_env(&env);
  std::fs::write(paths.socket(), b"").expect("socket");
  std::fs::write(paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  assert!(!super::replace_engine(
    &paths,
    std::time::Duration::from_millis(100)
  ));
  assert_eq!(ensure(&env), Ok(()));
}

#[test]
fn start_leaves_a_live_engine_in_place() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env_at(tmp.path());
  let epic = tmp.path().join("epic");
  std::fs::create_dir_all(&epic).expect("epic");
  let paths = Paths::from_env(&env);
  std::fs::write(paths.lock(), format!("{}\n", std::process::id())).expect("lock");
  let outcome = start(&verb(&["epic", "start", &epic.display().to_string()]), &env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  assert!(paths.registry().is_file());
  let settings = |safe: bool, no_jev: bool| crate::epics::LaunchSettings { safe, no_jev };
  assert_eq!(crate::epics::launch_settings(&epic), settings(false, false));
  let dir = epic.display().to_string();
  let outcome = start(&verb(&["epic", "start", &dir, "--safe", "--no-jev"]), &env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  assert_eq!(crate::epics::launch_settings(&epic), settings(true, true));
  let again = start(&verb(&["epic", "start", &dir]), &env);
  assert_eq!(again.exit, Exit::Success, "{again:?}");
  assert_eq!(crate::epics::launch_settings(&epic), settings(false, false));
  std::fs::remove_file(epic.join("launch.json")).expect("settings");
  std::fs::create_dir_all(epic.join("launch.json")).expect("a directory in the way");
  let unwritable = start(&verb(&["epic", "start", &dir, "--safe"]), &env);
  assert_eq!(unwritable.exit, Exit::Failure, "{unwritable:?}");
  let other = tmp.path().join("other");
  std::fs::create_dir_all(other.join("launch.json")).expect("a directory in the way");
  let other_dir = other.display().to_string();
  let refused = start(&verb(&["epic", "start", &other_dir]), &env);
  assert_eq!(refused.exit, Exit::Failure, "{refused:?}");
  let registry = read_value(&paths.registry()).expect("registry").to_string();
  assert!(!registry.contains(&other_dir), "{registry}");
}

#[test]
fn start_rejects_a_missing_directory_and_a_bad_registry() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = env_at(tmp.path());
  let missing = Command::new("epic")
    .subcommand(Command::new("start").arg(Arg::new("state-dir")))
    .get_matches_from(["epic", "start"]);
  let outcome = start(missing.subcommand().expect("start").1, &env);
  assert!(
    outcome
      .stderr
      .expect("err")
      .contains("state-dir is required")
  );
  let epic = tmp.path().join("epic");
  let dir = epic.display().to_string();
  let absent = start(&verb(&["epic", "start", &dir]), &env);
  assert_eq!(
    absent.stderr.as_deref(),
    Some(format!("toolu epic start: no epic state directory {dir}").as_str())
  );
  assert!(!epic.exists());
  std::fs::create_dir_all(&epic).expect("epic");
  let paths = Paths::from_env(&env);
  std::fs::create_dir_all(paths.registry()).expect("dir");
  let outcome = start(&verb(&["epic", "start", &dir]), &env);
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(outcome.stderr.expect("err").contains("toolu epic start"));
  // The settings come first, so the epic is never registered without them.
  assert!(epic.join("launch.json").is_file());
}

fn verb(argv: &[&str]) -> clap::ArgMatches {
  let matches = crate::verbs::command()
    .try_get_matches_from(argv)
    .expect("parse");
  matches.subcommand().expect("subcommand").1.clone()
}

fn env_at(tmp: &Path) -> Env {
  let root = tmp.join("resources");
  std::fs::create_dir_all(&root).expect("root");
  Env::from_pairs([("TOOLU_RESOURCE_HOME", root.display().to_string())])
}

#[test]
fn attention_options_follow_the_epic_attention_settings() {
  let tmp = tempfile::tempdir().expect("temp");
  let config = tmp.path().join("config");
  let env = |config: &Path| {
    Env::from_pairs([
      ("HOME", tmp.path().display().to_string()),
      ("TOOLU_CONFIG_DIR", config.display().to_string()),
    ])
  };
  let quiet = attention_options(&env(&config), None).expect("defaults");
  assert_eq!(quiet.remind_ms, 30 * 60_000);
  assert!(quiet.notify.is_none());
  std::fs::create_dir_all(&config).expect("config dir");
  let body = r#"{"version":1,"epic":{"attention":{"enabled":true,"remind_after_minutes":5}}}"#;
  std::fs::write(config.join("toolu.config.json"), body).expect("config");
  let http = crate::http::Config {
    addr: "0.0.0.0:7800".parse().expect("addr"),
    secrets: toolu_runtime::config::secrets::Secrets::none(),
  };
  let options = attention_options(&env(&config), Some(&http)).expect("configured");
  assert_eq!(options.remind_ms, 5 * 60_000);
  let notify = options.notify.expect("notifier");
  assert_eq!((notify.post, notify.herdr), (true, false));
  assert_eq!(notify.dashboard.as_deref(), Some("http://127.0.0.1:7800/"));
}
