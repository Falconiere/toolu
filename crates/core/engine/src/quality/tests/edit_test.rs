use serde_json::{Map, Value};
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};

use super::{edited_file, edited_record};

#[test]
fn quality_edit_prefers_the_edit_path_and_resolves_it() {
  let mut input = Map::new();
  input.insert("path".to_owned(), Value::String("src/a.ts".to_owned()));
  input.insert("file_path".to_owned(), Value::String("src/b.ts".to_owned()));
  let text = |value| Text::new(value).unwrap();
  let session = Session {
    session_id: text("s"),
    cwd: text("/tmp/project"),
    project_root: text("/tmp/project"),
    worktree: text("/tmp/project"),
  };
  let event = NormalizedEvent::ToolPost {
    session,
    tool: Tool {
      call_id: text("t"),
      name: text("Edit"),
      input,
    },
    output: None,
  };
  let env = Env::default();
  let raw = Map::new();
  let root = std::path::Path::new("/tmp/project");
  let ctx = RuleContext {
    host: toolu_protocol::host::Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let file = edited_file(&event, &ctx).expect("Edit names a file");
  assert_eq!(file.path, "src/a.ts");
  assert_eq!(file.absolute, root.join("src/a.ts"));
  assert!(!file.removed);
}

#[test]
fn quality_edit_empty_move_target_keeps_the_source_live() {
  let env = Env::default();
  let raw = Map::new();
  let root = std::path::Path::new("/tmp/project");
  let ctx = RuleContext {
    host: toolu_protocol::host::Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let record = EditRecord {
    path: "src/a.ts".to_owned(),
    operation: EditOperation::Update,
    moved_to: Some(String::new()),
    from: None,
  };
  let file = edited_record(&record, &ctx).unwrap();
  assert!(!file.removed);
}
