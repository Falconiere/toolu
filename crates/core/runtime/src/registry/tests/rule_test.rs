use std::path::Path;

use serde_json::{Map, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;

use super::{EditOperation, EditSplit, Rule, RuleContext};
use crate::env::Env;
use crate::registry::RegistryEvent;

/// Denies edits to the path its context names, as a quality rule would.
struct DenyMoves;

impl Rule for DenyMoves {
  fn spec(&self) -> &'static str {
    "demo@toolu"
  }

  fn name(&self) -> &'static str {
    "deny-moves"
  }

  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPre
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    match (event, ctx.edit) {
      (NormalizedEvent::ToolPre { .. }, Some(split)) if split.operation == EditOperation::Move => {
        Decision::Deny {
          reason: Text::new(format!("{} moved to {}", split.from, split.moved_to)).unwrap(),
        }
      }
      _ => Decision::Allow,
    }
  }
}

#[test]
fn a_rule_decides_from_the_event_and_its_context() {
  let event: NormalizedEvent = serde_json::from_value(json!({
    "type": "tool/pre", "sessionId": "s", "cwd": "/p", "projectRoot": "/p", "worktree": "/p",
    "toolCallId": "c", "toolName": "apply_patch", "toolInput": {}
  }))
  .unwrap();
  let env = Env::from_pairs([("HOME", "/h")]);
  let raw = Map::new();
  let mut ctx = RuleContext {
    host: Host::Codex,
    env: &env,
    config_root: Path::new("/h/.codex"),
    project_root: Path::new("/p"),
    cwd: Some(Path::new("/p")),
    plugin_root: None,
    raw: &raw,
    edit: Some(EditSplit {
      operation: EditOperation::Move,
      from: "a.ts",
      moved_to: "b.ts",
    }),
  };
  let rules: Vec<Box<dyn Rule>> = vec![Box::new(DenyMoves)];
  let rule = rules.first().unwrap();
  assert_eq!(
    (rule.spec(), rule.name(), rule.event()),
    ("demo@toolu", "deny-moves", RegistryEvent::ToolPre)
  );
  let deny = Decision::Deny {
    reason: Text::new("a.ts moved to b.ts").unwrap(),
  };
  assert_eq!(rule.run(&event, &ctx), deny);
  ctx.edit = Some(EditSplit {
    operation: EditOperation::Update,
    from: "a.ts",
    moved_to: "",
  });
  assert_eq!(rule.run(&event, &ctx), Decision::Allow);
  assert!(
    rule.applies(&event, &ctx),
    "a rule applies unless it says otherwise"
  );
}

/// Applies only to the `.ts` path its split names, as a language rule would.
struct TypeScriptOnly;

impl Rule for TypeScriptOnly {
  fn spec(&self) -> &'static str {
    "ts-quality@toolu"
  }

  fn name(&self) -> &'static str {
    "ts-quality"
  }

  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPost
  }

  fn applies(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    let tool = event.tool();
    let path = tool.and_then(|tool| tool.input.get("file_path")?.as_str());
    path.is_some_and(|path| Path::new(path).extension() == Some("ts".as_ref()))
  }

  fn run(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Decision {
    Decision::Allow
  }
}

#[test]
fn a_rule_can_decline_an_event_before_it_runs() {
  let edit = |path: &str| -> NormalizedEvent {
    serde_json::from_value(json!({
      "type": "tool/post", "sessionId": "s", "cwd": "/p", "projectRoot": "/p", "worktree": "/p",
      "toolCallId": "c", "toolName": "Edit", "toolInput": {"file_path": path}
    }))
    .unwrap()
  };
  let env = Env::from_pairs([("HOME", "/h")]);
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: Path::new("/h/.claude"),
    project_root: Path::new("/p"),
    cwd: None,
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  assert!(TypeScriptOnly.applies(&edit("/p/a.ts"), &ctx));
  assert!(!TypeScriptOnly.applies(&edit("/p/a.py"), &ctx));
}
