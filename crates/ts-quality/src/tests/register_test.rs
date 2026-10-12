use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::{BRIDGE_GUARD, bridge_script, quoted, register};

const MANIFEST: &str = "toolu/post-tools.d/ts-quality@toolu__ts-quality.json";
const BRIDGE: &str = "toolu/post-tools.d/ts-quality@toolu__ts-quality.sh";

fn config(dir: &Path) -> Env {
  Env::from_pairs([("TOOLU_CONFIG_DIR", dir.display().to_string())])
}

#[test]
fn register_writes_the_manifest_and_bridge_and_is_stable() {
  let dir = tempfile::tempdir().unwrap();
  let env = config(dir.path());
  let first = register(&env, Host::Claude);
  assert_eq!((first.exit, first.stdout), (Exit::Success, None));
  let manifest = std::fs::read_to_string(dir.path().join(MANIFEST)).unwrap();
  assert_eq!(
    manifest,
    "{\"version\":1,\"spec\":\"ts-quality@toolu\",\"name\":\"ts-quality\",\"event\":\"tool/post\",\"matcher\":\"*\"}\n"
  );
  let bridge = std::fs::read_to_string(dir.path().join(BRIDGE)).unwrap();
  assert!(
    bridge.contains("ts-quality hook post-tools --event PostToolUse"),
    "{bridge}"
  );
  let modified = || {
    std::fs::metadata(dir.path().join(BRIDGE))
      .and_then(|meta| meta.modified())
      .unwrap()
  };
  let before = modified();
  assert_eq!(register(&env, Host::Claude).stdout, None);
  assert_eq!(modified(), before);
}

#[test]
fn register_removes_the_bun_module_with_a_notice_once() {
  let dir = tempfile::tempdir().unwrap();
  let env = config(dir.path());
  let post = dir.path().join("toolu/post-tools.d");
  let pre = dir.path().join("toolu/pre-tools.d");
  std::fs::create_dir_all(&post).unwrap();
  std::fs::create_dir_all(&pre).unwrap();
  std::fs::write(post.join("ts-quality@toolu__ts-quality.js"), "old").unwrap();
  std::fs::write(pre.join("ts-quality@toolu__other.sh"), "old").unwrap();
  std::fs::write(post.join("rust-quality@toolu__rust-quality.js"), "kept").unwrap();
  let first = register(&env, Host::Codex);
  assert_eq!(
    first.stdout.as_deref(),
    Some(
      r#"{"systemMessage":"ts-quality's Bun module was replaced by its native rule in toolu."}"#
    )
  );
  assert!(!post.join("ts-quality@toolu__ts-quality.js").exists());
  assert!(!pre.join("ts-quality@toolu__other.sh").exists());
  assert!(post.join("rust-quality@toolu__rust-quality.js").exists());
  assert_eq!(register(&env, Host::Codex).stdout, None);
}

#[test]
fn register_reports_its_manifest_and_survives_a_bad_root() {
  let dir = tempfile::tempdir().unwrap();
  let report = dir.path().join("report.jsonl");
  std::fs::write(&report, "").unwrap();
  let env = config(dir.path()).with("TOOLU_STARTUP_REPORT", &report.display().to_string());
  assert_eq!(register(&env, Host::Opencode).exit, Exit::Success);
  let text = std::fs::read_to_string(&report).unwrap();
  let record: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
  assert_eq!(record["kind"], "native-registry");
  let target = dir.path().join(MANIFEST).display().to_string();
  assert_eq!(record["target"], target);
  let missing = config(dir.path()).with("TOOLU_STARTUP_REPORT", "/nonexistent/report");
  let unreported = register(&missing, Host::Opencode)
    .stdout
    .unwrap_or_default();
  assert!(
    unreported.contains("cannot write startup report /nonexistent/report"),
    "{unreported}"
  );
  let file = dir.path().join("plain-file");
  std::fs::write(&file, "").unwrap();
  let failed = register(&config(&file), Host::Claude);
  assert_eq!(failed.exit, Exit::Success);
  let message = failed.stdout.unwrap_or_default();
  assert!(
    message.contains("ts-quality registration failed: "),
    "{message}"
  );
}

fn bridge(script: &Path, envs: &[(&str, &str)]) -> Output {
  let mut command = Command::new("/bin/sh");
  command.arg(script).env_clear().env("PATH", "/usr/bin:/bin");
  command.envs(envs.iter().copied()).output().unwrap()
}

#[test]
fn the_bridge_runs_the_native_hook_once_for_typescript_payloads() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("it's toolu");
  std::fs::write(&bin, format!("#!/bin/sh\necho \"${BRIDGE_GUARD}:$*\"\n")).unwrap();
  std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
  assert_eq!(quoted("it's"), r"'it'\''s'");
  let script = dir.path().join("bridge.sh");
  std::fs::write(&script, bridge_script(&bin)).unwrap();
  let ran = bridge(&script, &[("input", r#"{"file_path":"a.ts"}"#)]);
  assert_eq!(
    String::from_utf8_lossy(&ran.stdout),
    "1:ts-quality hook post-tools --event PostToolUse\n"
  );
  let paths = bridge(&script, &[("CLAUDE_FILE_PATHS", "b.tsx")]);
  assert_ne!(paths.stdout.len(), 0);
  let python = bridge(&script, &[("input", r#"{"file_path":"a.py"}"#)]);
  assert_eq!(python.stdout.len(), 0);
  let nested = bridge(&script, &[("input", "a.ts"), (BRIDGE_GUARD, "1")]);
  assert_eq!(nested.stdout.len(), 0);
  std::fs::write(&script, bridge_script(&dir.path().join("missing"))).unwrap();
  let lost = bridge(&script, &[("input", "a.ts")]);
  assert_eq!(lost.status.code(), Some(1));
  assert_eq!(
    String::from_utf8_lossy(&lost.stderr),
    "ts-quality: toolu not found; TypeScript checks skipped\n"
  );
}
