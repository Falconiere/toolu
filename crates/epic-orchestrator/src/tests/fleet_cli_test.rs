use std::path::Path;

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::config::secrets::Secrets;
use toolu_runtime::env::Env;

use crate::fleet_rig::{Engine, config, free_port, machine};
use crate::peer_fixture::{closed, serve};

fn home(root: &Path, epic: &Value, tokens: &str) -> Env {
  let config_dir = root.join("config");
  std::fs::create_dir_all(&config_dir).expect("config dir");
  let body = json!({"version": 1, "epic": epic});
  std::fs::write(config_dir.join("toolu.config.json"), body.to_string()).expect("config");
  Env::from_pairs([
    ("HOME", root.display().to_string()),
    ("TOOLU_CONFIG_DIR", config_dir.display().to_string()),
    (
      "TOOLU_RESOURCE_HOME",
      root.join("resources").display().to_string(),
    ),
    ("TOOLU_EPIC_PEER_TOKENS", tokens.to_owned()),
  ])
}

fn fleet(env: &Env) -> toolu_runtime::cli::Outcome {
  let matches = crate::verbs::command()
    .try_get_matches_from(["epic", "status", "--fleet"])
    .expect("parse");
  let (_, status) = matches.subcommand().expect("status");
  crate::query::status(status, env)
}

#[test]
fn fleet_cli_lists_peers() {
  let temp = tempfile::tempdir().expect("temp root");
  let b_config = config(free_port(), Vec::new(), Secrets::none());
  let b = Engine::start(&temp.path().join("b"), "repo-500", "repo-501", b_config);
  let peers = json!([
    {"name": "server", "url": format!("http://{}", b.addr)},
    {"name": "nas", "url": format!("http://{}", closed())},
    {"name": "local", "url": format!("http://{}", b.addr)},
  ]);
  let env = home(&temp.path().join("cli"), &json!({"peers": peers}), "");
  let outcome = fleet(&env);
  assert_eq!(outcome.exit, Exit::Success, "{:?}", outcome.stderr);
  let doc: Value = serde_json::from_str(&outcome.stdout.expect("stdout")).expect("json");
  let local = &doc["machines"][0];
  assert_eq!(
    (local["local"].clone(), local["engine"].clone()),
    (json!(true), json!("down"))
  );
  assert_eq!(machine(&doc, "server")["in_flight"][0]["key"], "repo-501");
  assert_eq!(machine(&doc, "nas")["state"], "unreachable");
  assert_eq!(machine(&doc, "nas")["last_seen"], Value::Null);
  let named_local = &doc["machines"][3];
  assert_eq!(
    (named_local["name"].clone(), named_local["local"].clone()),
    (json!("local"), json!(false))
  );
  assert_eq!(doc["summary"]["machines"], 4);
  assert_eq!(doc["summary"]["reachable"], 3);
  b.stop();
}

#[test]
fn fleet_cli_conflicts_with_epic() {
  let error = crate::verbs::command()
    .try_get_matches_from(["epic", "status", "--fleet", "repo-402"])
    .expect_err("conflict");
  assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn fleet_cli_rejects_credential_config() {
  let temp = tempfile::tempdir().expect("temp root");
  let epic = json!({"peers": [{"name": "server", "url": "http://127.0.0.1:1", "token": "x"}]});
  let outcome = fleet(&home(temp.path(), &epic, ""));
  assert_eq!(outcome.exit, Exit::Failure);
  let stderr = outcome.stderr.expect("stderr");
  assert!(
    stderr.starts_with("toolu epic status --fleet: epic.peers[0].token"),
    "{stderr}"
  );
}

#[test]
fn fleet_token_canary_cli() {
  let temp = tempfile::tempdir().expect("temp root");
  let canary = "PEER-TOKEN-CANARY-3";
  let document = json!({"schema": 1, "engine": canary,
    "issues": [{"key": canary, "epic": "e", "stage": "running", "phase": canary}]});
  let echo = serve(200, "application/json", &document.to_string());
  let refused = serve(
    401,
    "application/json",
    r#"{"schema":1,"error":"unauthorized"}"#,
  );
  let peers = json!([
    {"name": "echo", "url": format!("http://{}", echo.addr)},
    {"name": "refused", "url": format!("http://{}", refused.addr)},
  ]);
  let tokens = json!({"echo": canary, "refused": format!("{canary}-B")}).to_string();
  let outcome = fleet(&home(temp.path(), &json!({"peers": peers}), &tokens));
  assert_eq!(outcome.exit, Exit::Success);
  let printed = format!("{:?}{:?}", outcome.stdout, outcome.stderr);
  assert!(!printed.contains(canary), "{printed}");
  assert!(printed.contains("<redacted>"), "{printed}");
  let head = echo.seen().to_ascii_lowercase();
  assert!(head.contains(&format!(
    "authorization: bearer {}",
    canary.to_ascii_lowercase()
  )));
  assert!(refused.seen().to_ascii_lowercase().contains("-b"));
}
