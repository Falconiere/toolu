use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;

use super::{jev_client, launch};

fn run(dir: &std::path::Path, extra: &[&str], json: bool) -> toolu_runtime::cli::Outcome {
  let mut graph = crate::support::graph_248();
  graph.state_dir = dir.join("state").display().to_string();
  let file = dir.join("graph.json");
  crate::graph::write_graph(&file, &graph).expect("graph");
  let file = file.display().to_string();
  let mut line = vec!["epic", "launch", "--graph", &file];
  line.extend_from_slice(extra);
  let all = crate::verbs::command()
    .try_get_matches_from(&line)
    .expect("line");
  let env = Env::from_pairs([
    ("HOME", dir.display().to_string()),
    (
      "HERDR_SOCKET_PATH",
      dir.join("none.sock").display().to_string(),
    ),
  ]);
  launch(
    all.subcommand_matches("launch").expect("launch"),
    &Ctx {
      json,
      ..Ctx::default()
    },
    &env,
  )
}

#[test]
fn a_dry_run_prints_the_plan_and_a_refusal_its_report() {
  let tmp = tempfile::tempdir().expect("temp");
  let text = run(tmp.path(), &["--issue", "comemory-255", "--dry-run"], false)
    .stdout
    .expect("text");
  assert!(text.starts_with("# Falconiere/comemory#255\n"), "{text}");
  let json = run(
    tmp.path(),
    &["--issue", "comemory-255", "--dry-run", "--kind", "codex"],
    true,
  );
  let doc: serde_json::Value = serde_json::from_str(&json.stdout.expect("json")).expect("document");
  assert_eq!(doc["host"]["kind"], "codex");
  let blocked = run(tmp.path(), &["--issue", "comemory-257", "--dry-run"], false);
  assert_eq!(blocked.exit, Exit::Failure);
  let report: serde_json::Value =
    serde_json::from_str(&blocked.stderr.expect("report")).expect("json");
  assert_eq!(
    (report["issue"].clone(), report["native_exit_code"].clone()),
    (serde_json::json!("comemory-257"), serde_json::Value::Null)
  );
  assert!(jev_client(&Env::from_pairs([("HOME", "/tmp")])).is_err());
}

#[test]
fn a_manual_launch_in_a_safe_epic_keeps_approval_prompts_on() {
  let tmp = tempfile::tempdir().expect("temp");
  let line = ["--issue", "comemory-255", "--dry-run"];
  let unattended = run(tmp.path(), &line, false).stdout.expect("text");
  assert!(
    unattended.contains("--dangerously-skip-permissions"),
    "{unattended}"
  );
  let settings = crate::epics::LaunchSettings {
    safe: true,
    no_jev: false,
  };
  crate::epics::write_launch_settings(&tmp.path().join("state"), settings).expect("settings");
  let safe = run(tmp.path(), &line, false).stdout.expect("text");
  assert!(!safe.contains("--dangerously-skip-permissions"), "{safe}");
}
