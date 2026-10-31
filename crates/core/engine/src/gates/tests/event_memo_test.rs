use std::cell::Cell;
use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analyze;

use super::super::bash_commands::BASH_COMMANDS;
use super::super::commit_gate::COMMIT_GATE;
use super::super::docs_sync::DOCS_SYNC;
use super::super::gate_status::GATE_STATUS;
use super::super::plan_ledger::PLAN_LEDGER;
use super::super::protected_files::PROTECTED;
use super::super::push_review::PUSH_REVIEW;
use super::super::push_waiver::PUSH_WAIVER;
use super::super::quality_gate::QUALITY_GATE;
use super::super::{command_analysis, gate_config, pre_mode};
use super::EventMemoGuard;
use crate::builtins::PRE_TOOL;
use crate::dispatch::{DEFAULT_MODULE_TIMEOUT, DispatchOptions, dispatch_pre_tool};
use crate::docs_review::RULE;
use crate::gate::Gate;
use crate::ledger::context::test_repo::Repo;

fn reset_counts() {
  super::PARSES.with(|count| count.set(0));
  super::LOADS.with(|count| count.set(0));
}

fn analysis_parses() -> usize {
  super::PARSES.with(Cell::get)
}

fn config_loads() -> usize {
  super::LOADS.with(Cell::get)
}

fn text(value: &str) -> Text {
  Text::new(value).expect("text")
}

fn shell(command: &str) -> (NormalizedEvent, Map<String, Value>) {
  let raw = json!({"tool_input": {"command": command}})
    .as_object()
    .expect("object")
    .clone();
  let event = NormalizedEvent::ShellPre {
    session: Session {
      session_id: text("s"),
      cwd: text("cwd"),
      project_root: text("root"),
      worktree: text("tree"),
    },
    tool: Tool {
      call_id: text("c"),
      name: text("Bash"),
      input: Map::new(),
    },
    command: text(command),
  };
  (event, raw)
}

fn tool_post(command: &str) -> (NormalizedEvent, Map<String, Value>) {
  let raw = json!({"tool_input": {"command": command}})
    .as_object()
    .expect("object")
    .clone();
  let event = NormalizedEvent::ToolPost {
    session: Session {
      session_id: text("s"),
      cwd: text("cwd"),
      project_root: text("root"),
      worktree: text("tree"),
    },
    tool: Tool {
      call_id: text("c"),
      name: text("Bash"),
      input: Map::new(),
    },
    output: None,
  };
  (event, raw)
}

fn context<'a>(
  env: &'a Env,
  root: &'a Path,
  raw: &'a Map<String, Value>,
  cwd: Option<&'a Path>,
  host: Host,
) -> RuleContext<'a> {
  RuleContext {
    host,
    env,
    config_root: root,
    project_root: root,
    cwd,
    plugin_root: None,
    raw,
    edit: None,
  }
}

fn pre_gates() -> [&'static dyn Gate; 5] {
  [
    &COMMIT_GATE,
    &DOCS_SYNC,
    &PLAN_LEDGER,
    &PUSH_REVIEW,
    &QUALITY_GATE,
  ]
}

fn run_pre(event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Vec<Result<Decision, String>> {
  pre_gates()
    .into_iter()
    .map(|gate| gate.run(event, ctx))
    .collect()
}

#[test]
fn one_event_shares_one_analysis_of_a_compound_command() {
  let dir = tempfile::tempdir().expect("tempdir");
  let home = dir.path().to_str().expect("utf8");
  let env = Env::from_pairs([("HOME", home)]);
  let command = "echo 'a b' > out.txt && printf '%s' x";
  let (event, raw) = shell(command);
  let ctx = context(&env, dir.path(), &raw, Some(dir.path()), Host::Claude);
  reset_counts();
  let uncached = run_pre(&event, &ctx);
  assert_eq!(analysis_parses(), 5);
  assert_eq!(config_loads(), 0);
  reset_counts();
  let cached = {
    let _guard = EventMemoGuard::enter();
    run_pre(&event, &ctx)
  };
  assert_eq!(analysis_parses(), 1);
  assert_eq!(uncached, cached);
  reset_counts();
  let broken = "echo \"unterminated";
  let (_event, raw) = shell(broken);
  let ctx = context(&env, dir.path(), &raw, Some(dir.path()), Host::Claude);
  let _guard = EventMemoGuard::enter();
  let analysis = command_analysis(&ctx);
  assert!(analysis.unknown);
  assert_eq!(analysis, analyze(broken));
  let _again = command_analysis(&ctx);
  assert_eq!(analysis_parses(), 1);
}

#[test]
fn a_real_pre_tool_dispatch_parses_the_command_once() {
  let dir = tempfile::tempdir().expect("tempdir");
  std::fs::create_dir_all(dir.path().join(".claude")).expect("config dir");
  std::fs::write(
    dir.path().join(".claude/toolu.config.json"),
    "{\"version\":1}\n",
  )
  .expect("config");
  let home = dir.path().to_str().expect("utf8");
  let env = Env::from_pairs([("HOME", home)]);
  let options = DispatchOptions {
    env: &env,
    cwd: dir.path(),
    lib_dir: Path::new("/lib"),
    builtins: PRE_TOOL,
    rules: &[],
    quality_rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    split_by_caller: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  };
  let payload = concat!(
    r#"{"session_id":"s1","hook_event_name":"PreToolUse","tool_name":"Bash","#,
    r#""tool_input":{"command":"echo 'a b' > out.txt && printf '%s' x"}}"#
  );
  reset_counts();
  let dispatched = dispatch_pre_tool(payload, &options);
  assert_eq!(
    (dispatched.result.exit_code, analysis_parses()),
    (0, 1),
    "stderr {} trace {:?}",
    dispatched.result.stderr,
    dispatched.trace
  );
}

#[test]
fn invalid_config_is_loaded_once_and_each_push_gate_keeps_the_warning() {
  let repo = Repo::new().expect("repo");
  let config = repo.root.join(".home/toolu.config.json");
  std::fs::write(&config, "{\"version\":2}\n").expect("config");
  let env = repo.env();
  let (event, raw) = shell("git push origin HEAD:feat/x");
  let ctx = context(&env, &repo.root, &raw, Some(&repo.root), Host::Claude);
  reset_counts();
  let _guard = EventMemoGuard::enter();
  let mut plan_warnings = Vec::new();
  let mut review_warnings = Vec::new();
  COMMIT_GATE.run(&event, &ctx).expect("commit");
  DOCS_SYNC.run(&event, &ctx).expect("docs");
  PLAN_LEDGER
    .run_warning(&event, &ctx, &mut plan_warnings)
    .expect("plan");
  PUSH_REVIEW
    .run_warning(&event, &ctx, &mut review_warnings)
    .expect("review");
  QUALITY_GATE.run(&event, &ctx).expect("quality");
  let loaded = gate_config(&ctx);
  assert_eq!((analysis_parses(), config_loads()), (1, 1));
  assert!(loaded.invalid.is_some(), "{loaded:?}");
  assert_eq!(pre_mode(&loaded, "pushReview", &ctx, true), GateMode::Block);
  assert!(
    plan_warnings
      .iter()
      .any(|line| line.contains("failing closed")),
    "{plan_warnings:?}"
  );
  assert!(
    review_warnings
      .iter()
      .any(|line| line.contains("failing closed")),
    "{review_warnings:?}"
  );
}

#[test]
fn a_new_walk_does_not_reuse_analysis_or_config() {
  let dir = tempfile::tempdir().expect("tempdir");
  let home = dir.path().join("home");
  std::fs::create_dir(&home).expect("home");
  let config = home.join("toolu.config.json");
  std::fs::write(&config, "{\"version\":2}\n").expect("config");
  let home_text = home.to_str().expect("utf8");
  let env = Env::from_pairs([("HOME", home_text), ("TOOLU_CONFIG_DIR", home_text)]);
  let (_event, raw) = shell("echo hi");
  let ctx = context(&env, dir.path(), &raw, Some(dir.path()), Host::Claude);
  reset_counts();
  {
    let _guard = EventMemoGuard::enter();
    assert_eq!(command_analysis(&ctx).source, "echo hi");
    assert!(gate_config(&ctx).invalid.is_some());
    assert_eq!((analysis_parses(), config_loads()), (1, 1));
  }
  std::fs::write(&config, "{\"not-a-key\":true}\n").expect("rewrite");
  {
    let _guard = EventMemoGuard::enter();
    let (_next, next_raw) = shell("echo next");
    let next_ctx = context(&env, dir.path(), &next_raw, Some(dir.path()), Host::Claude);
    assert_eq!(command_analysis(&next_ctx).source, "echo next");
    let invalid = gate_config(&next_ctx).invalid.expect("invalid");
    assert!(invalid.contains("not-a-key"), "{invalid}");
    assert_eq!((analysis_parses(), config_loads()), (2, 2));
  }
  {
    let _guard = EventMemoGuard::enter();
    let _same = command_analysis(&ctx);
    assert_eq!(analysis_parses(), 3);
  }
}

#[test]
fn host_cwd_and_repository_changes_load_config_again() {
  let root = tempfile::tempdir().expect("tempdir");
  let first = root.path().join("a");
  let second = root.path().join("b");
  std::fs::create_dir(&first).expect("a");
  std::fs::create_dir(&second).expect("b");
  std::fs::write(first.join("toolu.config.json"), "{\"version\":2}\n").expect("a config");
  std::fs::write(second.join("toolu.config.json"), "{\"version\":3}\n").expect("b config");
  let a_text = first.to_str().expect("utf8");
  let b_text = second.to_str().expect("utf8");
  let env_a = Env::from_pairs([
    ("HOME", a_text),
    ("TOOLU_CONFIG_DIR", a_text),
    ("TOOLU_PROJECT_DIR", a_text),
  ]);
  let env_b = Env::from_pairs([
    ("HOME", b_text),
    ("TOOLU_CONFIG_DIR", b_text),
    ("TOOLU_PROJECT_DIR", b_text),
  ]);
  let (_event, raw) = shell("echo hi");
  reset_counts();
  let _guard = EventMemoGuard::enter();
  let claude = context(&env_a, &first, &raw, Some(&first), Host::Claude);
  let loaded = gate_config(&claude);
  assert!(loaded.invalid.is_some());
  let codex = context(&env_a, &first, &raw, Some(&first), Host::Codex);
  assert_eq!(gate_config(&codex).host, Host::Codex);
  let other_cwd = context(&env_a, &first, &raw, Some(&second), Host::Claude);
  let _cwd = gate_config(&other_cwd);
  let other_repo = context(&env_b, &second, &raw, Some(&second), Host::Claude);
  let moved = gate_config(&other_repo);
  assert_eq!(config_loads(), 4, "host, cwd, and repository each load");
  assert!(
    moved.files.user.starts_with(&second),
    "{}",
    moved.files.user.display()
  );
  assert!(
    moved
      .invalid
      .as_deref()
      .is_some_and(|text| text.contains("version")),
    "{moved:?}"
  );
}

#[test]
fn empty_lists_skip_parsing_and_a_deny_rule_parses_once() {
  let dir = tempfile::tempdir().expect("tempdir");
  let settings = dir.path().join("settings");
  std::fs::create_dir(&settings).expect("settings");
  let home = dir.path().to_str().expect("utf8");
  let settings_text = settings.to_str().expect("utf8");
  let env = Env::from_pairs([("HOME", home), ("TOOLU_SETTINGS_DIR", settings_text)]);
  let (event, raw) = shell("echo hi");
  let ctx = context(&env, dir.path(), &raw, Some(dir.path()), Host::Claude);
  reset_counts();
  {
    let _guard = EventMemoGuard::enter();
    assert!(matches!(
      BASH_COMMANDS.run(&event, &ctx).expect("bash"),
      Decision::Allow
    ));
    assert!(matches!(
      PROTECTED.run(&event, &ctx).expect("protected"),
      Decision::Allow
    ));
    assert_eq!(analysis_parses(), 0);
  }
  std::fs::write(settings.join("bash-denylist.txt"), "rm -rf /\n").expect("deny");
  reset_counts();
  let _guard = EventMemoGuard::enter();
  assert!(matches!(
    BASH_COMMANDS.run(&event, &ctx).expect("bash"),
    Decision::Allow
  ));
  assert!(matches!(
    BASH_COMMANDS.run(&event, &ctx).expect("bash again"),
    Decision::Allow
  ));
  assert_eq!(analysis_parses(), 1);
}

#[test]
fn post_tool_gates_and_docs_review_share_within_their_event() {
  let dir = tempfile::tempdir().expect("tempdir");
  let home = dir.path().to_str().expect("utf8");
  let env = Env::from_pairs([("HOME", home)]);
  let (post, post_raw) = tool_post("cargo test");
  let post_ctx = context(&env, dir.path(), &post_raw, Some(dir.path()), Host::Claude);
  reset_counts();
  {
    let _guard = EventMemoGuard::enter();
    GATE_STATUS.run(&post, &post_ctx).expect("status");
    PUSH_WAIVER.run(&post, &post_ctx).expect("waiver");
    assert_eq!(analysis_parses(), 1);
  }
  let (event, raw) = shell("echo hi");
  let ctx = context(&env, dir.path(), &raw, Some(dir.path()), Host::Claude);
  reset_counts();
  let _guard = EventMemoGuard::enter();
  let _review = RULE.run(&event, &ctx);
  COMMIT_GATE.run(&event, &ctx).expect("commit");
  assert_eq!(analysis_parses(), 1);
}
