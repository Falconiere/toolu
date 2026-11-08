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

use epic::{Home, Res, err};
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
  for (kind, first) in [
    ("claude", "--dangerously-skip-permissions"),
    ("codex", "--no-daemon"),
    ("cursor", "--yolo"),
    ("muse", "--yolo"),
    ("grok", "--always-approve"),
  ] {
    let args = vec![
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
    let result = doc(&home.run(&args).unwrap()).unwrap();
    assert_eq!(result["agent_args"][0], first, "{kind}");
    assert_eq!(result["resume"], false);
    assert!(
      result["prompt"]
        .as_str()
        .is_some_and(|prompt| prompt.starts_with("You are an epic worker."))
    );
    assert!(result.get("exclude").is_none(), "{kind}");
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

/// One host's resume: its captured session, its first resumed args, its
/// attended args from index 2, and how its brief names delivery-flow.
struct Resume {
  kind: &'static str,
  session: &'static str,
  lead: [&'static str; 3],
  safe: &'static [&'static str],
  skill: &'static str,
}

/// The dry-run documents of a resume of `host`'s captured session, without
/// and with `--safe`.
fn resume_dry_runs(host: &Resume) -> Res<(serde_json::Value, serde_json::Value)> {
  let home = Home::new()?;
  let graph = write_graph(&home, |_| {})?;
  let record = home.path("state/issues/comemory-255.json");
  std::fs::create_dir_all(record.parent().ok_or("parent")?)?;
  let body = format!(
    r#"{{"stage":"running","kind":"{}","launches":1,"session_id":"{}"}}"#,
    host.kind, host.session
  );
  std::fs::write(&record, body)?;
  let launch = |extra: &[&str]| -> Res<serde_json::Value> {
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
      host.kind,
    ];
    args.extend_from_slice(extra);
    Ok(doc(&home.run(&args)?)?)
  };
  Ok((launch(&[])?, launch(&["--safe"])?))
}

#[test]
fn muse_and_grok_resumes_dry_run_their_captured_session_and_skill_syntax() {
  let muse = Resume {
    kind: "muse",
    session: "01a16abb-7eef-7340-8cf2-2e0378dcf66c",
    lead: ["resume", "01a16abb-7eef-7340-8cf2-2e0378dcf66c", "--yolo"],
    safe: &["--approval-mode", "on-request", "--trust-workspace"],
    skill: "the `plugin:delivery-flow:delivery-flow` skill",
  };
  let grok = Resume {
    kind: "grok",
    session: "01a1bd42-396e-7cb3-9e06-4c45a5eda666",
    lead: [
      "--resume",
      "01a1bd42-396e-7cb3-9e06-4c45a5eda666",
      "--always-approve",
    ],
    safe: &["--permission-mode", "default"],
    skill: "`/delivery-flow:delivery-flow`",
  };
  for host in [muse, grok] {
    let (resumed, safe) = resume_dry_runs(&host).unwrap();
    assert_eq!(resumed["resume"], true);
    assert_eq!(
      resumed["agent_args"].as_array().expect("args")[..3],
      host.lead
    );
    let end = 2 + host.safe.len();
    assert_eq!(
      safe["agent_args"].as_array().expect("args")[2..end],
      *host.safe
    );
    assert!(
      resumed["brief"]
        .as_str()
        .is_some_and(|brief| brief.contains(host.skill)),
      "{}",
      host.kind
    );
  }
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
