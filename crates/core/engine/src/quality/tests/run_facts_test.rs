use std::path::PathBuf;
use std::process::Command;

use serde_json::Map;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::edit_records::{EditOperation, EditRecord};

use super::tests::context;
use super::{AstGrepScan, EditedFile, QualityFindings, QualityRule, run_quality};

struct NamedRule {
  source: &'static str,
  skip_linked: bool,
}

impl QualityRule for NamedRule {
  fn language(&self) -> &'static str {
    "fixture"
  }
  fn extensions(&self) -> &[&str] {
    &["fx"]
  }
  fn source(&self) -> &str {
    self.source
  }
  fn reason(&self) -> &'static str {
    "fixture failed"
  }
  fn project_enabled(&self, _ctx: &RuleContext<'_>) -> bool {
    true
  }
  fn skip_linked_worktrees(&self) -> bool {
    self.skip_linked
  }
  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }
  fn check(
    &self,
    file: &EditedFile,
    _ctx: &RuleContext<'_>,
    _scan: &AstGrepScan,
  ) -> QualityFindings {
    let errors = std::fs::read_to_string(&file.absolute)
      .ok()
      .filter(|text| text.contains("bad"))
      .map_or_else(Vec::new, |_| vec!["bad".to_owned()]);
    QualityFindings {
      errors,
      advisories: Vec::new(),
    }
  }
}

fn git(dir: &std::path::Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("git starts");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

fn record(path: &str, operation: EditOperation) -> EditRecord {
  EditRecord {
    path: path.to_owned(),
    operation,
    moved_to: None,
    from: None,
  }
}

#[test]
fn one_file_two_rules_discover_repository_once_when_git_fallback_is_required() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  git(root, &["init", "-q"]);
  std::fs::write(root.join("a.fx"), "bad\n").unwrap();
  let trace = root.join("git-trace.jsonl");
  let env = Env::process()
    .with("GIT_DISCOVERY_ACROSS_FILESYSTEM", "true")
    .with("GIT_TRACE2_EVENT", &trace.to_string_lossy());
  let raw = Map::new();
  let ctx = context(root, &env, &raw);
  let first = NamedRule {
    source: "first",
    skip_linked: true,
  };
  let second = NamedRule {
    source: "second",
    skip_linked: false,
  };
  let outcome = run_quality(
    &[record("a.fx", EditOperation::Write)],
    &ctx,
    &[&first, &second],
  );
  assert_eq!(outcome.decisions.len(), 2);
  let starts: Vec<Vec<String>> = std::fs::read_to_string(trace)
    .unwrap()
    .lines()
    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .filter(|event| event["event"] == "start")
    .filter_map(|event| {
      event["argv"].as_array().map(|args| {
        args
          .iter()
          .filter_map(|arg| arg.as_str().map(str::to_owned))
          .collect()
      })
    })
    .collect();
  let linked = starts
    .iter()
    .filter(|args| args.iter().any(|arg| arg == "--git-common-dir"))
    .count();
  let toplevel = starts
    .iter()
    .filter(|args| args.iter().any(|arg| arg == "--show-toplevel"))
    .count();
  assert_eq!((linked, toplevel), (1, 2), "{starts:?}");
}

#[test]
fn nested_repository_and_next_event_use_the_current_owner() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  git(root, &["init", "-q"]);
  let nested = root.join("nested");
  std::fs::create_dir(&nested).unwrap();
  git(&nested, &["init", "-q"]);
  std::fs::write(root.join("parent.fx"), "bad\n").unwrap();
  std::fs::write(nested.join("child.fx"), "bad\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(root, &env, &raw);
  let rule = NamedRule {
    source: "first",
    skip_linked: false,
  };
  let records = [
    record("parent.fx", EditOperation::Write),
    record("nested/child.fx", EditOperation::Write),
  ];
  let first = run_quality(&records, &ctx, &[&rule]);
  assert_eq!(first.decisions.len(), 2);
  let parent_gate = root.join(".claude/tmp/quality-gate-status.json");
  let nested_gate = nested.join(".claude/tmp/quality-gate-status.json");
  let parent_doc: serde_json::Value =
    serde_json::from_slice(&std::fs::read(&parent_gate).unwrap()).unwrap();
  let nested_doc: serde_json::Value =
    serde_json::from_slice(&std::fs::read(&nested_gate).unwrap()).unwrap();
  assert!(parent_doc["entries"]["parent.fx"].is_object());
  assert!(nested_doc["entries"]["nested/child.fx"].is_object());
  assert!(parent_doc["entries"]["nested/child.fx"].is_null());
  std::fs::remove_dir_all(nested.join(".git")).unwrap();
  let second = run_quality(&records[1..], &ctx, &[&rule]);
  assert_eq!(second.decisions.len(), 1);
  let parent_doc: serde_json::Value =
    serde_json::from_slice(&std::fs::read(parent_gate).unwrap()).unwrap();
  assert!(parent_doc["entries"]["nested/child.fx"].is_object());
}

#[test]
fn removed_and_nonregular_paths_are_rechecked_on_the_next_event() {
  let temp = tempfile::tempdir().unwrap();
  let root = temp.path();
  git(root, &["init", "-q"]);
  std::fs::write(root.join("gone.fx"), "bad\n").unwrap();
  let env = Env::process();
  let raw = Map::new();
  let ctx = context(root, &env, &raw);
  let rule = NamedRule {
    source: "first",
    skip_linked: false,
  };
  let write = record("gone.fx", EditOperation::Write);
  assert_eq!(
    run_quality(std::slice::from_ref(&write), &ctx, &[&rule])
      .decisions
      .len(),
    1
  );
  std::fs::remove_file(root.join("gone.fx")).unwrap();
  assert_eq!(
    run_quality(&[record("gone.fx", EditOperation::Delete)], &ctx, &[&rule])
      .decisions
      .len(),
    0
  );
  let gate = root.join(".claude/tmp/quality-gate-status.json");
  let cleared: serde_json::Value = serde_json::from_slice(&std::fs::read(&gate).unwrap()).unwrap();
  assert_eq!(cleared["status"], "passing");
  std::fs::create_dir(root.join("gone.fx")).unwrap();
  assert_eq!(
    run_quality(std::slice::from_ref(&write), &ctx, &[&rule])
      .decisions
      .len(),
    0
  );
  std::fs::remove_dir(root.join("gone.fx")).unwrap();
  std::fs::write(root.join("gone.fx"), "bad\n").unwrap();
  assert_eq!(run_quality(&[write], &ctx, &[&rule]).decisions.len(), 1);
}
