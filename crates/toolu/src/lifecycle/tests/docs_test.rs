use serde_json::{Map, Value};
use toolu_protocol::host::Host;
use toolu_runtime::config::load::LoadedConfig;

use super::{DocInput, doc_parts, event_title, render_doc};
use crate::lifecycle::project::ProjectFacts;

#[test]
fn titles_match_the_bash_branches() {
  assert_eq!(event_title("startup"), "Toolu is on!");
  assert_eq!(event_title("resume"), "Session resumed");
  assert_eq!(event_title("clear"), "Context cleared");
  assert_eq!(event_title("compact"), "Context compacted");
  assert_eq!(event_title("reboot"), "");
}

#[test]
fn tokens_are_replaced_literally_and_a_missing_file_is_empty() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("note.md");
  std::fs::write(&path, "Hello {{project_name}} $ &\n\n").unwrap();
  assert_eq!(
    render_doc(&path, &[("project_name", "A$&B")]),
    "Hello A$&B $ &"
  );
  assert_eq!(render_doc(&dir.path().join("missing.md"), &[]), "");
}

#[test]
fn doc_parts_fill_tokens_and_skip_routing_when_models_are_off() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join("session-start.md"),
    "PROTO {{project_name}}|{{model_mechanical}}|{{effort_mechanical}}",
  )
  .unwrap();
  std::fs::write(dir.path().join("model-routing.md"), "ROUTE").unwrap();
  std::fs::write(dir.path().join("post-compaction.md"), "AFTER").unwrap();
  std::fs::write(dir.path().join("session-start-rust.md"), "RUST").unwrap();
  let facts = ProjectFacts {
    name: "demo".to_owned(),
    node_pm: String::new(),
    rust: true,
    ts: false,
    python: false,
  };
  let on = LoadedConfig::from_data(Map::new(), Host::Claude);
  let joined = doc_parts(&DocInput {
    docs: dir.path(),
    event: "startup",
    config: &on,
    host: Host::Claude,
    facts: &facts,
    verbose: false,
  })
  .join("\n");
  assert_eq!(joined, "PROTO demo|haiku|\nROUTE");
  let mut models = Map::new();
  models.insert("enabled".to_owned(), Value::Bool(false));
  let mut data = Map::new();
  data.insert("models".to_owned(), Value::Object(models));
  let off = LoadedConfig::from_data(data, Host::Claude);
  let skipped = doc_parts(&DocInput {
    docs: dir.path(),
    event: "compact",
    config: &off,
    host: Host::Codex,
    facts: &facts,
    verbose: true,
  });
  assert_eq!(
    skipped,
    vec![
      "AFTER".to_owned(),
      "PROTO demo|gpt-5.6-luna|, effort `medium`".to_owned(),
      "RUST".to_owned(),
    ]
  );
}
