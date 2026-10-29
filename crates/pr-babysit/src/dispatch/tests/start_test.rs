use serde_json::json;
use toolu_runtime::process::{Spec, run};

use super::*;
use crate::dispatch::dispatcher::Options;
use crate::fixture;

const MINIMAL: &str =
  r#"{"version":2,"repo":"Falconiere/toolu","number":165,"slot":"falconiere-toolu-165"}"#;

fn claude_plan(dir: &Path) -> std::path::PathBuf {
  one_group_plan(dir, "claude", "opus", "xhigh")
}

fn one_group_plan(dir: &Path, host: &str, model: &str, effort: &str) -> std::path::PathBuf {
  let items = fixture::json("items/review-items.json");
  let id = items["items"][0]["id"].clone();
  let group = json!({"seq": 1, "tier": "critical", "host": host, "model": model, "effort": effort, "items": [id]});
  let plan = dir.join("plan.json");
  std::fs::write(
    &plan,
    json!({"dispatch": "herdr", "unattended": true, "groups": [group]}).to_string(),
  )
  .unwrap();
  plan
}

fn dry_start(state: &Path, plan: &Path, repo: &str) -> Value {
  let env = toolu_runtime::env::Env::from_pairs([("PATH", "/usr/bin:/bin")]);
  let toolu = Path::new("/opt/toolu/bin/toolu");
  let options = Options {
    state_file: state,
    env: &env,
    toolu,
    dry: true,
  };
  let items = fixture::path("items/review-items.json");
  let start = Start {
    plan,
    items: &items,
    repo_root: repo,
    branch: "feat/fix",
    base: "main",
    machine: None,
    remote_repo: None,
  };
  Dispatcher::new(&options).unwrap().start(&start).unwrap()
}

#[test]
fn a_dry_run_records_the_herdr_commands_and_keeps_the_state_byte_identical() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("state.json");
  std::fs::write(&state, MINIMAL).unwrap();
  let repo = dir.path().join("repo").display().to_string();
  let result = dry_start(&state, &claude_plan(dir.path()), &repo);
  assert_eq!(result["dryRun"], true);
  let heads: Vec<Vec<String>> = result["commands"]
    .as_array()
    .unwrap()
    .iter()
    .map(|command| {
      command
        .as_array()
        .unwrap()
        .iter()
        .take(3)
        .map(|part| part.as_str().unwrap().to_owned())
        .collect()
    })
    .collect();
  let git_head = vec!["git".to_owned(), "-C".into(), repo];
  let herdr = |verb: &str, sub: &str| vec!["herdr".to_owned(), verb.into(), sub.into()];
  let expected = vec![
    git_head.clone(),
    git_head.clone(),
    git_head,
    herdr("worktree", "create"),
    herdr("agent", "start"),
    herdr("agent", "prompt"),
  ];
  assert_eq!(heads, expected);
  let brief = result["brief"].as_str().unwrap();
  assert!(brief.contains("Falconiere/toolu#165"));
  assert!(brief.contains("'/opt/toolu/bin/toolu' babysit fixer-report"));
  assert!(
    brief.contains(
      fixture::json("items/review-items.json")["items"][0]["id"]
        .as_str()
        .unwrap()
    )
  );
  assert_eq!(std::fs::read_to_string(&state).unwrap(), MINIMAL);
  assert!(!dir.path().join("state.json.lock").exists());
}

#[test]
fn a_muse_group_starts_a_muse_agent_with_its_approval_model_and_effort_flags() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join("state.json");
  std::fs::write(&state, MINIMAL).unwrap();
  let repo = dir.path().join("repo").display().to_string();
  let plan = one_group_plan(dir.path(), "muse", "muse-wonder-full-1.0", "xhigh");
  let result = dry_start(&state, &plan, &repo);
  let start: Vec<&str> = result["commands"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(Value::as_array)
    .find(|command| command.get(2).and_then(Value::as_str) == Some("start"))
    .unwrap()
    .iter()
    .filter_map(Value::as_str)
    .collect();
  let tail = start.iter().position(|arg| *arg == "--").unwrap();
  assert_eq!(start[..3], ["herdr", "agent", "start"]);
  assert!(
    start[3].starts_with("pb-") && start[3].ends_with("-r1g1"),
    "{start:?}"
  );
  assert_eq!(start[4..6], ["--kind", "muse"]);
  assert_eq!(
    start[tail + 1..],
    [
      "--yolo",
      "--model",
      "muse-wonder-full-1.0",
      "--reasoning-effort",
      "xhigh"
    ]
  );
}

#[test]
fn the_brief_report_commands_survive_an_apostrophe_in_the_state_path() {
  let top = tempfile::tempdir().unwrap();
  let dir = top.path().join("owner's run");
  std::fs::create_dir(&dir).unwrap();
  let state = dir.join("state.json");
  std::fs::write(&state, MINIMAL).unwrap();
  let result = dry_start(
    &state,
    &claude_plan(&dir),
    &dir.join("repo").display().to_string(),
  );
  let brief = result["brief"].as_str().unwrap();
  let report = dir
    .join("state.fixer-r1g1.report.json")
    .display()
    .to_string();
  for (placeholder, status) in [
    ("\"<one-line summary>\"", "done"),
    ("\"<the reason>\"", "failed"),
  ] {
    let line = brief
      .lines()
      .find(|line| line.contains(placeholder))
      .unwrap();
    let command = line
      .trim()
      .trim_matches('`')
      .replace(placeholder, "'fixed'");
    let words = command.strip_prefix("'/opt/toolu/bin/toolu'").unwrap();
    let mut spec = Spec::new(["sh", "-c", &format!("printf '%s\\n'{words}")]);
    spec.env = Some(toolu_runtime::env::Env::from_pairs([(
      "PATH",
      "/usr/bin:/bin",
    )]));
    let printed = run(&spec).unwrap().stdout;
    let argv: Vec<&str> = printed.lines().collect();
    assert_eq!(
      argv,
      [
        "babysit",
        "fixer-report",
        report.as_str(),
        status,
        "--note",
        "fixed"
      ]
    );
  }
}
