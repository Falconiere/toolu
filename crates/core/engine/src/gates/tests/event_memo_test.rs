use std::cell::Cell;
use std::path::Path;

use serde_json::{Map, Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analyze;

use super::super::bash_commands::BASH_COMMANDS;
use super::super::command_analysis;
use super::super::commit_gate::COMMIT_GATE;
use super::super::docs_sync::DOCS_SYNC;
use super::super::gate_status::GATE_STATUS;
use super::super::plan_ledger::PLAN_LEDGER;
use super::super::protected_files::PROTECTED;
use super::super::push_review::PUSH_REVIEW;
use super::super::push_waiver::PUSH_WAIVER;
use super::super::quality_gate::QUALITY_GATE;
use super::EventMemoGuard;
use crate::builtins::PRE_TOOL;
use crate::dispatch::{DEFAULT_MODULE_TIMEOUT, DispatchOptions, dispatch_pre_tool};
use crate::docs_review::RULE;
use crate::gate::Gate;

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
