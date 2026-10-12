use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};

use super::ManifestRule;
use crate::quality::{AstGrepScan, EditedFile, QualityFindings, QualityRule};

/// Flags a `.lint` file holding `bad`; a gate entry records it.
struct Lint;

impl QualityRule for Lint {
  fn language(&self) -> &'static str {
    "lint"
  }
  fn extensions(&self) -> &[&str] {
    &["lint"]
  }
  fn source(&self) -> &'static str {
    "lint-hook"
  }
  fn reason(&self) -> &'static str {
    "lint says no"
  }
  fn project_enabled(&self, ctx: &RuleContext<'_>) -> bool {
    ctx.project_root.join("lint.toml").is_file()
  }
  fn skip_linked_worktrees(&self) -> bool {
    true
  }
  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }
  fn check(&self, file: &EditedFile, _: &RuleContext<'_>, _: &AstGrepScan) -> QualityFindings {
    let text = std::fs::read_to_string(&file.absolute).unwrap_or_default();
    let errors = if text.contains("bad") {
      vec![format!("bad in {}", file.path)]
    } else {
      vec![]
    };
    QualityFindings {
      errors,
      advisories: vec!["checked".to_owned()],
    }
  }
}

const RULE: &dyn Rule = &ManifestRule {
  spec: "lint@toolu",
  name: "lint",
  rule: Lint,
};

fn written(root: &Path, path: &str) -> NormalizedEvent {
  let at = Text::new(root.display().to_string()).unwrap();
  let mut input = Map::new();
  input.insert("file_path".to_owned(), Value::String(path.to_owned()));
  NormalizedEvent::ToolPost {
    session: Session {
      session_id: Text::new("s").unwrap(),
      cwd: at.clone(),
      project_root: at.clone(),
      worktree: at,
    },
    tool: Tool {
      call_id: Text::new("c").unwrap(),
      name: Text::new("Write").unwrap(),
      input,
    },
    output: None,
  }
}

#[test]
fn a_manifest_rule_applies_by_extension_and_runs_its_quality_rule() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  std::fs::write(root.join("lint.toml"), "").unwrap();
  std::fs::write(root.join("a.lint"), "bad\n").unwrap();
  std::fs::write(root.join("b.lint"), "good\n").unwrap();
  let home = root.join("home").display().to_string();
  let env = Env::from_pairs([("HOME", home.clone()), ("TOOLU_CONFIG_DIR", home)]);
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  assert_eq!(
    (RULE.spec(), RULE.name(), RULE.event()),
    ("lint@toolu", "lint", RegistryEvent::ToolPost)
  );
  assert!(!RULE.applies(&written(root, "a.txt"), &ctx));
  assert!(RULE.applies(&written(root, "a.lint"), &ctx));
  let mut warnings = Vec::new();
  let failing = RULE.run_warning(&written(root, "a.lint"), &ctx, &mut warnings);
  let Decision::Advisory { message } = failing else {
    panic!("{failing:?}");
  };
  assert_eq!(
    message.as_str(),
    "QUALITY VIOLATION — fix before proceeding:\nbad in a.lint\n"
  );
  assert_eq!(warnings.len(), 0, "{warnings:?}");
  let passing = RULE.run(&written(root, "b.lint"), &ctx);
  assert!(matches!(passing, Decision::Advisory { message } if message.as_str() == "checked"));
  std::fs::remove_file(root.join("lint.toml")).unwrap();
  assert_eq!(RULE.run(&written(root, "a.lint"), &ctx), Decision::Allow);
}
