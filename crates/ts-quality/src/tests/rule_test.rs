use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use serde_json::{Map, Value};
use toolu_engine::quality::{AstGrepScan, EditedFile, QualityRule};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::RuleContext;

use super::{TS_QUALITY, TsQuality};

fn write_event(root: &Path, path: &str) -> NormalizedEvent {
  let text = |value: &str| Text::new(value).unwrap();
  let at = root.display().to_string();
  let mut input = Map::new();
  input.insert("file_path".to_owned(), Value::String(path.to_owned()));
  NormalizedEvent::ToolPost {
    session: Session {
      session_id: text("s"),
      cwd: text(&at),
      project_root: text(&at),
      worktree: text(&at),
    },
    tool: Tool {
      call_id: text("t"),
      name: text("Write"),
      input,
    },
    output: None,
  }
}

/// A TypeScript repository at `root` with a `bun` stub on `PATH`.
fn project(root: &Path) -> Env {
  for (name, body) in [
    ("tsconfig.json", "{}\n"),
    ("package.json", "{}\n"),
    ("bun.lock", ""),
  ] {
    std::fs::write(root.join(name), body).unwrap();
  }
  for args in [&["init", "-q"][..], &["add", "tsconfig.json"]] {
    let status = Command::new("git").arg("-C").arg(root).args(args).status();
    assert!(status.unwrap().success());
  }
  let bin = root.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  std::fs::write(bin.join("bun"), "#!/bin/sh\n").unwrap();
  std::fs::set_permissions(bin.join("bun"), std::fs::Permissions::from_mode(0o755)).unwrap();
  let home = root.join("home").display().to_string();
  Env::from_pairs([
    ("PATH", format!("{}:/usr/bin:/bin", bin.display())),
    ("HOME", home.clone()),
    ("TOOLU_CONFIG_DIR", home),
  ])
}

fn context<'a>(env: &'a Env, raw: &'a Map<String, Value>, root: &'a Path) -> RuleContext<'a> {
  RuleContext {
    host: Host::Claude,
    env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw,
    edit: None,
  }
}

fn decide(env: &Env, root: &Path, path: &str) -> (Decision, Vec<String>) {
  let raw = Map::new();
  let ctx = context(env, &raw, root);
  let event = write_event(root, path);
  assert!(TS_QUALITY.applies(&event, &ctx), "{path}");
  let mut warnings = Vec::new();
  (
    TS_QUALITY.run_warning(&event, &ctx, &mut warnings),
    warnings,
  )
}

#[test]
fn the_rule_is_enabled_by_its_manifest_on_ts_edits() {
  assert_eq!(
    (TS_QUALITY.spec(), TS_QUALITY.name()),
    ("ts-quality@toolu", "ts-quality")
  );
  assert_eq!(TS_QUALITY.event(), RegistryEvent::ToolPost);
  assert_eq!(
    (TsQuality.language(), TsQuality.source()),
    ("ts", "ts-quality-hook")
  );
  assert_eq!(TsQuality.extensions(), ["ts", "tsx"]);
  assert_eq!(
    TsQuality.reason(),
    "Post-edit quality violation(s) detected"
  );
  assert!(TsQuality.skip_linked_worktrees() && TsQuality.ast_rule_dirs().is_empty());
  let env = Env::default();
  let raw = Map::new();
  let root = Path::new("/p");
  let ctx = context(&env, &raw, root);
  assert!(!TS_QUALITY.applies(&write_event(root, "a.py"), &ctx));
  assert!(!TsQuality.project_enabled(&ctx));
  assert!(TsQuality.ast_rules_inline(&ctx).contains("id: empty-catch"));
  let file = EditedFile {
    path: "a.ts".to_owned(),
    absolute: root.join("a.ts"),
    removed: false,
  };
  assert_eq!(
    TsQuality
      .check(&file, &ctx, &AstGrepScan::Missing)
      .errors
      .len(),
    0
  );
}

#[test]
fn a_violation_is_reported_and_a_clean_file_passes() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let env = project(root);
  std::fs::create_dir_all(root.join("src")).unwrap();
  std::fs::write(root.join("src/bad.ts"), "console.log(\"x\");\n").unwrap();
  let (decision, warnings) = decide(&env, root, "src/bad.ts");
  assert!(warnings.is_empty(), "{warnings:?}");
  let Decision::Advisory { message } = decision else {
    panic!("expected an advisory, got {decision:?}");
  };
  assert!(
    message
      .as_str()
      .contains("Forbidden console.log in src/bad.ts"),
    "{message:?}"
  );
  std::fs::write(root.join("src/bad.ts"), "/** A. */\nexport const A = 1;\n").unwrap();
  assert_eq!(decide(&env, root, "src/bad.ts").0, Decision::Allow);
}

#[test]
fn an_unreadable_file_fails_its_check() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let env = project(root);
  let raw = Map::new();
  let ctx = context(&env, &raw, root);
  let file = EditedFile {
    path: "gone.ts".to_owned(),
    absolute: root.join("gone.ts"),
    removed: false,
  };
  let findings = TsQuality.check(&file, &ctx, &AstGrepScan::Missing);
  assert_eq!(findings.errors.len(), 1);
  assert!(findings.errors[0].starts_with("Cannot read gone.ts for TypeScript quality checks: "));
}
