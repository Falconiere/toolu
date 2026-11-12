use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::{Rule, RuleContext};

use crate::dispatch::{DEFAULT_MODULE_TIMEOUT, DispatchOptions, dispatch_post_tool};
use crate::quality::{
  AstGrepScan, EditedFile, ManifestRule, QualityBinding, QualityFindings, QualityRule,
};

struct RustRule;

impl QualityRule for RustRule {
  fn language(&self) -> &'static str {
    "rust"
  }
  fn extensions(&self) -> &[&str] {
    &["rs"]
  }
  fn source(&self) -> &'static str {
    "fixture-quality"
  }
  fn reason(&self) -> &'static str {
    "fixture failed"
  }
  fn project_enabled(&self, _: &RuleContext<'_>) -> bool {
    true
  }
  fn skip_linked_worktrees(&self) -> bool {
    false
  }
  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }
  fn check(&self, file: &EditedFile, _: &RuleContext<'_>, _: &AstGrepScan) -> QualityFindings {
    QualityFindings {
      errors: vec![format!("checked {}", file.path)],
      advisories: Vec::new(),
    }
  }
}

struct Project {
  _temp: tempfile::TempDir,
  root: PathBuf,
  env: Env,
  log: PathBuf,
}

impl Project {
  fn new() -> Self {
    let temp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temp.path()).unwrap();
    assert!(
      Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .unwrap()
        .success()
    );
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let git = Command::new("sh")
      .args(["-c", "command -v git"])
      .output()
      .unwrap();
    let real_git = String::from_utf8(git.stdout).unwrap();
    let log = root.join("git.log");
    let wrapper = bin.join("git");
    std::fs::write(
      &wrapper,
      format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        real_git.trim()
      ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let env = Env::from_pairs([
      ("PATH", path),
      ("HOME", root.join("home").to_string_lossy().into_owned()),
      ("CLAUDE_PROJECT_DIR", root.to_string_lossy().into_owned()),
      ("TOOLU_HOST_OVERRIDE", "claude".to_owned()),
    ]);
    let registry = root.join("home/.claude/toolu/post-tools.d");
    std::fs::create_dir_all(&registry).unwrap();
    std::fs::write(
      registry.join("fixture@toolu__fixture.json"),
      r#"{"version":1,"spec":"fixture@toolu","name":"fixture","event":"tool/post","matcher":"*"}"#,
    )
    .unwrap();
    Self {
      _temp: temp,
      root,
      env,
      log,
    }
  }

  fn write(&self, name: &str) {
    std::fs::write(self.root.join(name), "fn main() {}\n").unwrap();
  }

  fn dispatch(&self, command: &str) -> crate::dispatch::ModuleResult {
    let rule = ManifestRule {
      spec: "fixture@toolu",
      name: "fixture",
      rule: RustRule,
    };
    let rules: [&dyn Rule; 1] = [&rule];
    let bindings = [QualityBinding {
      spec: rule.spec,
      name: rule.name,
      rule: &rule.rule,
    }];
    let options = DispatchOptions {
      env: &self.env,
      cwd: &self.root,
      lib_dir: Path::new("/lib"),
      builtins: &[],
      rules: &rules,
      quality_rules: &bindings,
      selected_specs: None,
      continue_post_blocks: false,
      split_by_caller: false,
      module_timeout: DEFAULT_MODULE_TIMEOUT,
    };
    let payload =
      serde_json::json!({"tool_name":"Bash", "tool_input":{"command":command}, "session_id":"s"});
    dispatch_post_tool(&payload.to_string(), &options).result
  }
}

#[test]
fn marker_excludes_preexisting_dirty_files_and_checks_later_changes() {
  let project = Project::new();
  project.write("old.rs");
  project.dispatch("touch marker");
  project.write("new.rs");
  let result = project.dispatch("sed -i s/a/b/ new.rs");
  assert!(result.stdout.contains("checked new.rs"), "{result:?}");
  assert!(!result.stdout.contains("checked old.rs"), "{result:?}");
}

#[test]
fn shell_checks_twenty_files_and_advises_about_the_other_five() {
  let project = Project::new();
  project.dispatch("touch marker");
  for index in 0..25 {
    project.write(&format!("file{index:02}.rs"));
  }
  let result = project.dispatch("sed -i s/a/b/ *.rs");
  assert_eq!(
    result.stdout.matches("checked file").count(),
    20,
    "{result:?}"
  );
  for index in 20..25 {
    assert!(
      result.stdout.contains(&format!("file{index:02}.rs")),
      "{result:?}"
    );
  }
  assert_eq!(result.stdout.matches("5 ").count(), 1, "{result:?}");
}

#[test]
fn read_only_shell_skips_git_status_and_mutation_runs_it_once() {
  let project = Project::new();
  project.write("x.rs");
  project.dispatch("cat x.rs");
  assert_eq!(
    std::fs::read_to_string(&project.log).unwrap_or_default(),
    ""
  );
  project.dispatch("sed -i s/a/b/ x.rs");
  let log = std::fs::read_to_string(&project.log).unwrap_or_default();
  assert_eq!(
    log
      .lines()
      .filter(|line| line.split_whitespace().any(|word| word == "status"))
      .count(),
    1,
    "{log}"
  );
}
