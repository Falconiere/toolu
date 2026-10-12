use serde_json::{Value, json};

use super::{Recovery, find_issue, launch_issue, render_dry, resolve_host};
use crate::launch_rig::{CLAUDE_SESSION, Rig, opts, snapshot_tree};
use crate::route::LaunchRoute;

fn dry(
  rig: &Rig,
  wanted: &str,
  tweak: impl FnOnce(&mut super::LaunchOpts),
) -> Result<Value, super::Failure> {
  let mut flags = opts();
  flags.dry_run = true;
  tweak(&mut flags);
  let issue = find_issue(&rig.graph, wanted)?;
  launch_issue(&rig.graph, issue, &flags, &mut rig.ctx())
}

fn line<'a>(result: &'a Value, prefix: &str) -> Option<&'a str> {
  result["commands"]
    .as_array()?
    .iter()
    .filter_map(Value::as_str)
    .find(|line| line.starts_with(prefix))
}

#[test]
fn a_dry_run_prints_the_herdr_sequence_and_writes_nothing() {
  let rig = Rig::new();
  let before = snapshot_tree(rig.tmp.path());
  let result = dry(&rig, "Falconiere/comemory#255", |_| {}).expect("dry");
  let text = render_dry(&result);
  assert!(text.starts_with("# Falconiere/comemory#255\n"));
  assert!(line(&result, "git -C ").is_some_and(|line| line.ends_with("fetch origin main")));
  assert!(
    line(&result, "herdr worktree create").is_some_and(|line| line.contains("--base origin/main"))
  );
  assert!(line(&result, "herdr agent start comemory-255 --kind claude").is_some());
  assert!(line(&result, "herdr agent prompt comemory-255").is_some());
  assert!(
    text.contains("# brief -> ") && text.contains("# Epic worker brief — Falconiere/comemory#255")
  );
  assert_eq!(result["exclude"], json!([]));
  assert_eq!(result["resume"], json!(false));
  assert_eq!(snapshot_tree(rig.tmp.path()), before);
  assert_eq!(rig.herdr.calls(), Vec::new());
}

#[test]
fn a_missing_checkout_is_cloned_into_the_clone_root() {
  let rig = Rig::new();
  let result = dry(&rig, "Falconiere/homebrew-tap#1", |flags| {
    flags.force = true;
  })
  .expect("dry");
  let clone = format!(
    "gh repo clone Falconiere/homebrew-tap {}/homebrew-tap",
    rig.graph.clone_root
  );
  assert_eq!(line(&result, "gh repo clone"), Some(clone.as_str()));
}

#[test]
fn a_routed_codex_host_gets_bypass_model_effort_and_its_skill_syntax() {
  let rig = Rig::new();
  let result = dry(&rig, "Falconiere/comemory#255", |flags| {
    flags.kind = Some("codex".into());
    flags.model = Some("gpt-6-sol".into());
    flags.effort = Some("medium".into());
  })
  .expect("dry");
  assert_eq!(
    line(&result, "herdr agent start"),
    Some(
      "herdr agent start comemory-255 --kind codex --pane '<root-pane>' --timeout 90000 -- --no-daemon --dangerously-bypass-approvals-and-sandbox --model gpt-6-sol -c model_reasoning_effort=medium"
    )
  );
  let brief = result["brief"].as_str().unwrap_or("");
  assert!(
    brief.contains("`$delivery-flow:delivery-flow`")
      && !brief.contains("`/delivery-flow:delivery-flow`")
  );
  assert_eq!(
    result["host"],
    json!({"kind": "codex", "model": "gpt-6-sol", "effort": "medium"})
  );
}

#[test]
fn safe_keeps_approval_prompts_on() {
  let rig = Rig::new();
  let result = dry(&rig, "Falconiere/comemory#255", |flags| {
    flags.kind = Some("cursor-agent".into());
    flags.safe = true;
  })
  .expect("dry");
  let start = line(&result, "herdr agent start").unwrap_or("");
  assert!(
    start.contains("--kind cursor --pane '<root-pane>' --timeout 90000 -- --trust"),
    "{start}"
  );
  assert!(!start.contains("--yolo"));
}

#[test]
fn an_opencode_worker_gets_provider_model_the_exclude_and_its_skill_ids() {
  let rig = Rig::new();
  let result = dry(&rig, "Falconiere/comemory#255", |flags| {
    flags.kind = Some("opencode".into());
    flags.model = Some("probe/scripted".into());
  })
  .expect("dry");
  let commands: Vec<&str> = result["commands"]
    .as_array()
    .expect("commands")
    .iter()
    .filter_map(Value::as_str)
    .collect();
  let at = |prefix: &str| {
    commands
      .iter()
      .position(|line| line.starts_with(prefix))
      .expect(prefix)
  };
  assert!(
    at("herdr worktree create")
      < at("# opencode: exclude /.opencode/toolu/state/ /.opencode/tmp/ in ")
  );
  assert!(at("# opencode: exclude") < at("herdr agent start"));
  assert_eq!(
    commands[at("herdr agent start")],
    "herdr agent start comemory-255 --kind opencode --pane '<root-pane>' --timeout 90000 -- --auto --model probe/scripted"
  );
  assert!(
    result["brief"]
      .as_str()
      .unwrap_or("")
      .contains("`skill({ name: \"delivery-flow-delivery-flow\" })`")
  );
  assert_eq!(result["exclude"].as_array().map(Vec::len), Some(3));
  assert!(!rig.state().join("issues").exists());
  let refused = dry(&rig, "Falconiere/comemory#255", |flags| {
    flags.kind = Some("opencode".into());
    flags.model = Some("sonnet".into());
  })
  .expect_err("model");
  assert_eq!(
    refused.message,
    "OpenCode model must be provider/model, got sonnet"
  );
}

#[test]
fn blocked_issues_and_null_routes_are_refused_before_any_mutation() {
  let rig = Rig::new();
  let blocked = dry(&rig, "Falconiere/comemory#257", |_| {}).expect_err("blocked");
  assert!(
    blocked.message.contains("is blocked"),
    "{}",
    blocked.message
  );
  let routes = rig.state().join("routes");
  std::fs::create_dir_all(&routes).expect("routes");
  std::fs::write(routes.join("comemory-255.json"), r#"{"host":null}"#).expect("route");
  let null = dry(&rig, "Falconiere/comemory#255", |flags| flags.force = true).expect_err("null");
  assert!(null.message.contains("no host capacity"));
  assert!(dry(&rig, "Falconiere/comemory#999", |_| {}).is_err());
  assert_eq!(
    find_issue(&rig.graph, "comemory-io-183")
      .map(|issue| issue.reference.as_str())
      .ok(),
    Some("CodaSignal/comemory.io#183")
  );
}

#[test]
fn a_route_supplies_model_and_effort_only_for_its_own_host() {
  let route = LaunchRoute {
    host: Some(crate::hosts::HostKind::Codex),
    model: Some("m".into()),
    effort: Some("high".into()),
  };
  let routed = resolve_host(&opts(), Some(&route)).expect("routed");
  assert_eq!(
    (routed.kind.name(), routed.model.as_deref()),
    ("codex", Some("m"))
  );
  let mut flags = opts();
  flags.kind = Some("claude".into());
  let other = resolve_host(&flags, Some(&route)).expect("other");
  assert_eq!((other.kind.name(), other.model), ("claude", None));
  assert_eq!(
    resolve_host(&opts(), None)
      .map(|host| host.kind.name())
      .ok(),
    Some("claude")
  );
}

#[test]
fn a_live_launch_reserves_starts_prompts_and_records_the_session() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  rig.claude_agents();
  let mut flags = opts();
  flags.force = true;
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let github = toolu_http_test_support::Fixture::start().expect("fixture");
  crate::support::ok(
    &github,
    "/repos/Falconiere/comemory",
    &json!({"default_branch": "main"}),
  );
  let client = crate::support::gh(&github);
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&client);
  let result = launch_issue(&rig.graph, &issue, &flags, &mut ctx);
  let result = result.unwrap_or_else(|err| panic!("{}", err.message));
  let record = rig.record("comemory-255");
  assert_eq!(record["stage"], "running");
  assert_eq!(record["session_id"], CLAUDE_SESSION);
  assert_eq!(record["lifecycle_outcome"], "started");
  assert_eq!(record["launches"], 1);
  assert_eq!(result["issue"], "Falconiere/comemory#255");
  assert_eq!(rig.leases(), 1);
  let prompted = rig
    .herdr
    .state()
    .agents
    .first()
    .map(|agent| agent.prompts.clone())
    .unwrap_or_default();
  assert!(
    prompted
      .first()
      .is_some_and(|text| text.starts_with("You are an epic worker. Read ")),
    "{prompted:?}"
  );
  let brief =
    std::fs::read_to_string(record["brief"].as_str().expect("brief")).expect("brief file");
  assert!(brief.contains("toolu epic report <phase> --status-file "));
  let mut again = flags.clone();
  again.recovery = Recovery::Fresh;
  let retained = launch_issue(&rig.graph, &issue, &again, &mut ctx).expect("retained");
  assert_eq!(retained["lifecycle_outcome"], "retained");
  assert_eq!(rig.leases(), 1);
}
