use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use serde_json::Map;
use toolu_engine::quality::{AstGrepScan, EditedFile, QualityRule};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::RuleContext;

use super::{PYTHON_QUALITY, PythonQuality};

/// A Python project at `root`; `python3` on `PATH` when `with_python`.
fn python_project(root: &Path, with_python: bool) -> Env {
  std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"\n").unwrap();
  let init = std::process::Command::new("git")
    .arg("init")
    .arg("-q")
    .arg(root)
    .status();
  assert!(init.unwrap().success());
  let bin = root.join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  if with_python {
    std::fs::write(bin.join("python3"), "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(bin.join("python3"), std::fs::Permissions::from_mode(0o755)).unwrap();
  }
  let config = root.join("config").display().to_string();
  Env::from_pairs([
    ("PATH", bin.display().to_string()),
    ("TOOLU_CONFIG_DIR", config),
  ])
}

fn checked(root: &Path, env: &Env, path: &str) -> (bool, Vec<String>, Vec<String>) {
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let file = EditedFile {
    path: path.to_owned(),
    absolute: root.join(path),
    removed: false,
  };
  let found = PythonQuality.check(&file, &ctx, &AstGrepScan::Missing);
  (
    PythonQuality.project_enabled(&ctx),
    found.errors,
    found.advisories,
  )
}

#[test]
fn the_rule_is_python_quality_on_py_files() {
  assert_eq!(
    (
      PYTHON_QUALITY.spec(),
      PYTHON_QUALITY.name(),
      PYTHON_QUALITY.event()
    ),
    (
      "python-quality@toolu",
      "python-quality",
      RegistryEvent::ToolPost
    )
  );
  assert_eq!(
    (PythonQuality.language(), PythonQuality.extensions()),
    ("python", &["py"][..])
  );
  assert_eq!(
    (PythonQuality.source(), PythonQuality.reason()),
    (
      "python-quality-hook",
      "Post-edit Python quality violation(s) detected"
    )
  );
  assert!(!PythonQuality.skip_linked_worktrees());
  assert_eq!(PythonQuality.ast_rule_dirs().len(), 0);
}

#[test]
fn violations_come_in_fragment_order_with_the_docs_advisory() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let env = python_project(root, true);
  let config = root.join(".claude/toolu.config.json");
  std::fs::create_dir_all(config.parent().unwrap()).unwrap();
  std::fs::write(
    &config,
    r#"{"version":1,"lang":{"python":{"maxFnLines":1}}}"#,
  )
  .unwrap();
  std::fs::write(
    root.join("bad.py"),
    "def a():\n    x = 1\n    return x  # noqa\n",
  )
  .unwrap();
  let (enabled, errors, advisories) = checked(root, &env, "bad.py");
  assert!(enabled);
  assert_eq!(errors.len(), 2, "{errors:?}");
  assert!(errors[0].starts_with("Forbidden suppression in bad.py"));
  assert!(errors[1].starts_with("Function too long in bad.py (>1 lines)"));
  assert_eq!(
    advisories,
    ["Public def/class missing a docstring in bad.py — add a concise one:\n1: a"]
  );
}

#[test]
fn no_python3_disables_the_rule_and_an_unreadable_file_is_an_error() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let env = python_project(root, false);
  let (enabled, errors, _) = checked(root, &env, "gone.py");
  assert!(!enabled);
  assert_eq!(errors.len(), 1);
  assert!(
    errors[0].starts_with("Cannot read gone.py for Python quality checks: "),
    "{errors:?}"
  );
}
