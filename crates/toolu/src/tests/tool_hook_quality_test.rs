use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_engine::DispatchOptions;
use toolu_engine::dispatch::DEFAULT_MODULE_TIMEOUT;
use toolu_engine::trace::StepStatus;
use toolu_runtime::env::Env;
use toolu_state::gate_file::{GateRead, read_gate_file};
use toolu_state::gate_schema::{GateEntry, GateFile};

use super::{Phase, QUALITY_RULES, RULES};

fn quality_sources(project: &Path) {
  std::fs::write(project.join("a.ts"), "throw \"bad\";\n").unwrap();
  std::fs::write(project.join("b.ts"), "export const b = 2;\n").unwrap();
  std::fs::write(
    project.join("pyproject.toml"),
    "[project]\nname = 'quality-test'\nversion = '1'\n",
  )
  .unwrap();
  std::fs::write(project.join("c.py"), "value = 3\n").unwrap();
  std::fs::write(
    project.join("Cargo.toml"),
    "[package]\nname = 'quality-test'\nversion = '0.1.0'\n",
  )
  .unwrap();
  std::fs::write(
    project.join("src.rs"),
    "//! A.\n/// B.\npub fn value() -> u8 { 4 }\n",
  )
  .unwrap();
}

fn quality_manifests(manifests: &Path) {
  for name in ["ts-quality", "python-quality", "rust-quality"] {
    std::fs::write(
      manifests.join(format!("{name}@toolu__{name}.json")),
      format!(
        r#"{{"version":1,"spec":"{name}@toolu","name":"{name}","event":"tool/post","matcher":"*"}}"#
      ),
    )
    .unwrap();
  }
}

fn quality_project(root: &Path) -> PathBuf {
  let project = root.join("project");
  let manifests = root.join("home/.claude/toolu/post-tools.d");
  for dir in [&project, &manifests, &root.join("bin")] {
    std::fs::create_dir_all(dir).unwrap();
  }
  assert!(
    Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .status()
      .unwrap()
      .success()
  );
  std::fs::write(
    project.join("package.json"),
    "{\"name\":\"quality-test\",\"version\":\"1\"}",
  )
  .unwrap();
  std::fs::write(
    project.join("package-lock.json"),
    "{\"name\":\"quality-test\",\"lockfileVersion\":3,\"packages\":{}}",
  )
  .unwrap();
  std::fs::write(project.join("tsconfig.json"), "{}").unwrap();
  assert!(
    Command::new("git")
      .args(["add", "tsconfig.json"])
      .current_dir(&project)
      .status()
      .unwrap()
      .success()
  );
  quality_sources(&project);
  quality_manifests(&manifests);
  project
}

fn counted_ast_grep(root: &Path, project: &Path) -> (Env, PathBuf) {
  let real = Command::new("sh")
    .args(["-c", "command -v ast-grep"])
    .output()
    .unwrap();
  assert!(real.status.success());
  let real = String::from_utf8(real.stdout).unwrap();
  let bin = root.join("bin");
  let count = root.join("scan-count");
  let wrapper = bin.join("ast-grep");
  std::fs::write(
    &wrapper,
    format!(
      "#!/bin/sh\nprintf 'scan\\n' >> '{}'\nexec '{}' \"$@\"\n",
      count.display(),
      real.trim()
    ),
  )
  .unwrap();
  std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
  let path = format!(
    "{}:{}",
    bin.display(),
    std::env::var("PATH").unwrap_or_default()
  );
  let env = Env::process()
    .with("HOME", &root.join("home").to_string_lossy())
    .with("CLAUDE_PROJECT_DIR", &project.to_string_lossy())
    .with("TOOLU_HOST_OVERRIDE", "claude")
    .with("PATH", &path);
  (env, count)
}

fn options<'a>(env: &'a Env, project: &'a Path) -> DispatchOptions<'a> {
  DispatchOptions {
    env,
    cwd: project,
    lib_dir: Path::new("/lib"),
    builtins: &[],
    rules: RULES,
    quality_rules: QUALITY_RULES,
    selected_specs: None,
    continue_post_blocks: false,
    split_by_caller: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  }
}

fn failing_entries(gate: &Path) -> Vec<(String, GateEntry)> {
  let GateRead::Ok(GateFile::Failing {
    entries: Some(entries),
    ..
  }) = read_gate_file(gate)
  else {
    panic!("missing quality gate");
  };
  entries
}

#[test]
fn production_multi_path_quality_uses_one_real_scan() {
  let temp = tempfile::tempdir().unwrap();
  let project = quality_project(temp.path());
  let (env, count) = counted_ast_grep(temp.path(), &project);
  let options = options(&env, &project);
  let patch = "*** Begin Patch\n*** Update File: a.ts\n@@\n-old\n+new\n*** Update File: b.ts\n@@\n-old\n+new\n*** Update File: c.py\n@@\n-old\n+new\n*** Update File: src.rs\n@@\n-old\n+new\n*** End Patch\n";
  let payload = serde_json::json!({"session_id":"quality", "cwd":project, "tool_name":"apply_patch", "tool_input":{"command":patch}}).to_string();
  let result = toolu_engine::dispatch::dispatch(Phase::Post, &payload, &options);
  assert_eq!(result.result.exit_code, 0, "{result:?}");
  assert_eq!(result.result.stderr, "", "{result:?}");
  assert_eq!(std::fs::read_to_string(count).unwrap(), "scan\n");
  let gate = project.join(".claude/tmp/quality-gate-status.json");
  let entries = failing_entries(&gate);
  assert_eq!(entries.len(), 1);
  assert_eq!(entries.first().map(|entry| entry.0.as_str()), Some("a.ts"));
  assert_eq!(
    entries.first().map(|entry| entry.1.source.as_str()),
    Some("ts-quality-hook")
  );
  let decided: Vec<&str> = result
    .trace
    .iter()
    .filter(|step| step.status == StepStatus::Decided)
    .map(|step| step.module.as_str())
    .collect();
  assert_eq!(
    decided,
    [
      "ts-quality@toolu__ts-quality.json",
      "ts-quality@toolu__ts-quality.json",
      "python-quality@toolu__python-quality.json",
      "rust-quality@toolu__rust-quality.json",
    ]
  );
}

#[test]
fn a_later_executable_edit_refreshes_the_next_paths_quality_scan() {
  let temp = tempfile::tempdir().unwrap();
  let project = quality_project(temp.path());
  std::fs::write(project.join("a.ts"), "export const a = 1;\n").unwrap();
  let module = temp
    .path()
    .join("home/.claude/toolu/post-tools.d/zz@toolu__rewrite.sh");
  std::fs::write(
    &module,
    "#!/bin/sh\nif [ ! -e .rewritten ]; then\n  printf 'throw \"later\";\\n' > b.ts\n  : > .rewritten\nfi\n",
  )
  .unwrap();
  std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o755)).unwrap();
  let (env, count) = counted_ast_grep(temp.path(), &project);
  let options = options(&env, &project);
  let patch = "*** Begin Patch\n*** Update File: a.ts\n@@\n-old\n+new\n*** Update File: b.ts\n@@\n-old\n+new\n*** End Patch\n";
  let payload = serde_json::json!({"session_id":"mutate", "cwd":project, "tool_name":"apply_patch", "tool_input":{"command":patch}}).to_string();
  let result = toolu_engine::dispatch::dispatch(Phase::Post, &payload, &options);
  assert_eq!(result.result.exit_code, 0, "{result:?}");
  assert_eq!(result.result.stderr, "", "{result:?}");
  assert_eq!(std::fs::read_to_string(&count).unwrap(), "scan\nscan\n");
  let gate = project.join(".claude/tmp/quality-gate-status.json");
  let entries = failing_entries(&gate);
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].0, "b.ts");
  assert_eq!(entries[0].1.source, "ts-quality-hook");
  assert!(entries[0].1.violations.contains("b.ts:1:throw \"later\";"));
  let unchanged = toolu_engine::dispatch::dispatch(Phase::Post, &payload, &options);
  assert_eq!(unchanged.result.exit_code, 0, "{unchanged:?}");
  assert_eq!(
    std::fs::read_to_string(count).unwrap(),
    "scan\nscan\nscan\n"
  );
}
