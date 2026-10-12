//! `toolu epic launch --dry-run` from the real binary: every host, fresh and
//! resume, and the refusals before ownership.

#[path = "helpers/epic.rs"]
mod epic;
#[path = "helpers/epic_doc.rs"]
mod epic_doc;
#[path = "helpers/epic_graph.rs"]
mod epic_graph;
#[path = "helpers/epic_out.rs"]
mod epic_out;

use epic::{Home, err};
use epic_doc::doc;
use epic_graph::write_graph;
use epic_out::out;

#[test]
fn a_dry_run_prints_the_commands_and_the_brief_and_writes_nothing() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  let text = home
    .run(&[
      "epic",
      "launch",
      "--graph",
      &graph,
      "--issue",
      "Falconiere/comemory#255",
      "--dry-run",
    ])
    .unwrap();
  assert_eq!(text.status.code(), Some(0), "{}", err(&text));
  let stdout = out(&text);
  assert!(stdout.starts_with("# Falconiere/comemory#255\n"));
  assert!(stdout.contains("herdr agent start comemory-255 --kind claude"));
  assert!(stdout.contains("toolu epic report <phase> --status-file "));
  assert!(!home.path("state").exists() && !home.path("resources").exists());
}

#[test]
fn every_host_dry_runs_its_agent_args_prompt_and_exclude() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  for (kind, model, first) in [
    ("claude", None, "--dangerously-skip-permissions"),
    ("codex", None, "--no-daemon"),
    ("cursor", None, "--yolo"),
    ("opencode", Some("p/m"), "--auto"),
  ] {
    let mut args = vec![
      "--json",
      "epic",
      "launch",
      "--graph",
      &graph,
      "--issue",
      "comemory-255",
      "--dry-run",
      "--kind",
      kind,
    ];
    if let Some(model) = model {
      args.extend(["--model", model]);
    }
    let result = doc(&home.run(&args).unwrap()).unwrap();
    assert_eq!(result["agent_args"][0], first, "{kind}");
    assert_eq!(result["resume"], false);
    assert!(
      result["prompt"]
        .as_str()
        .is_some_and(|prompt| prompt.starts_with("You are an epic worker."))
    );
    assert_eq!(
      result["exclude"].as_array().map(Vec::len),
      Some(if kind == "opencode" { 3 } else { 0 })
    );
  }
}

#[test]
fn a_resume_dry_runs_the_captured_session() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  let record = home.path("state/issues/comemory-255.json");
  std::fs::create_dir_all(record.parent().expect("parent")).expect("dir");
  std::fs::write(&record, r#"{"stage":"running","kind":"codex","launches":1,"session_id":"0199a272-0b4e-7cc3-8c4f-071c81275655"}"#).expect("record");
  let result = doc(
    &home
      .run(&[
        "--json",
        "epic",
        "launch",
        "--graph",
        &graph,
        "--issue",
        "comemory-255",
        "--dry-run",
        "--kind",
        "codex",
      ])
      .unwrap(),
  )
  .unwrap();
  assert_eq!(result["resume"], true);
  assert_eq!(result["agent_args"][0], "resume");
  assert!(
    result["prompt"]
      .as_str()
      .is_some_and(|prompt| prompt.contains("resuming interrupted work"))
  );
}

#[test]
fn refusals_print_the_issue_report_and_exit_1() {
  let home = Home::new().unwrap();
  let graph = write_graph(&home, |_| {}).unwrap();
  let blocked = home
    .run(&[
      "epic",
      "launch",
      "--graph",
      &graph,
      "--issue",
      "Falconiere/comemory#257",
      "--dry-run",
    ])
    .unwrap();
  assert_eq!(blocked.status.code(), Some(1));
  let report: serde_json::Value = serde_json::from_str(err(&blocked).trim()).expect("report");
  assert_eq!(report["issue"], "Falconiere/comemory#257");
  assert!(
    report["error"]
      .as_str()
      .is_some_and(|error| error.contains("is blocked"))
  );
  let routes = home.path("state/routes");
  std::fs::create_dir_all(&routes).expect("routes");
  std::fs::write(routes.join("comemory-255.json"), r#"{"host":null}"#).expect("route");
  let null = home
    .run(&[
      "epic",
      "launch",
      "--graph",
      &graph,
      "--issue",
      "comemory-255",
      "--force",
    ])
    .unwrap();
  assert_eq!(null.status.code(), Some(1));
  assert!(err(&null).contains("no host capacity"));
  assert!(!home.path("resources").exists());
}
