use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::{QualityPlugin, check_toolu, quoted};

const TS: QualityPlugin = QualityPlugin {
  plugin: "ts-quality",
  spec: "ts-quality@toolu",
  name: "ts-quality",
  path_glob: "*.ts*",
  guard: "TOOLU_TS_QUALITY_NATIVE",
  language: "TypeScript",
};
const BRIDGE_GUARD: &str = TS.guard;

fn register(env: &Env, host: Host) -> toolu_runtime::cli::Outcome {
  TS.register(env, host)
}

fn bridge_script(bin: &Path) -> String {
  TS.bridge_script(bin)
}

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

fn bridge(script: &Path, path: &str, envs: &[(&str, &str)]) -> Output {
  let mut command = Command::new("/bin/sh");
  command.arg(script).env_clear().env("PATH", path);
  command.current_dir(script.parent().unwrap());
  command.envs(envs.iter().copied()).output().unwrap()
}

fn stub(path: &Path, body: &str) {
  std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

const SYSTEM: &str = "/usr/bin:/bin";
const WRITE_TS: [(&str, &str); 2] = [("tool_name", "Write"), ("input", r#"{"file_path":"a.ts"}"#)];

#[test]
fn the_bridge_runs_the_native_hook_once_for_typescript_edits() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("it's toolu");
  stub(&bin, &format!("echo \"${BRIDGE_GUARD}:$*\""));
  assert_eq!(quoted("it's"), r"'it'\''s'");
  let script = dir.path().join("bridge.sh");
  std::fs::write(&script, bridge_script(&bin)).unwrap();
  let ran = bridge(&script, SYSTEM, &WRITE_TS);
  assert_eq!(
    String::from_utf8_lossy(&ran.stdout),
    "1:ts-quality hook post-tools --event PostToolUse\n"
  );
  let paths = bridge(&script, SYSTEM, &[("CLAUDE_FILE_PATHS", "b.tsx")]);
  assert_ne!(paths.stdout.len(), 0);
  let python = bridge(&script, SYSTEM, &[("tool_name", "Edit"), ("input", "a.py")]);
  assert_eq!(python.stdout.len(), 0);
  let shell = bridge(
    &script,
    SYSTEM,
    &[("tool_name", "Bash"), ("input", "cat a.ts")],
  );
  assert_eq!(shell.stdout.len(), 0);
  let nested = bridge(
    &script,
    SYSTEM,
    &[WRITE_TS[0], WRITE_TS[1], (BRIDGE_GUARD, "1")],
  );
  assert_eq!(nested.stdout.len(), 0);
}

#[test]
fn the_bridge_falls_back_only_to_a_native_toolu_on_path() {
  let dir = tempfile::tempdir().unwrap();
  let script = dir.path().join("bridge.sh");
  std::fs::write(&script, bridge_script(Path::new(""))).unwrap();
  let shelf = dir.path().join("bin");
  std::fs::create_dir(&shelf).unwrap();
  stub(
    &shelf.join("toolu"),
    "echo 'usage: toolu install' >&2; exit 2",
  );
  stub(&dir.path().join("toolu"), "echo cwd");
  let path = format!("{}:{SYSTEM}", shelf.display());
  for lost in [
    bridge(&script, &path, &WRITE_TS),
    bridge(&script, SYSTEM, &WRITE_TS),
  ] {
    assert_eq!(lost.status.code(), Some(1));
    assert_eq!(lost.stdout.len(), 0);
    assert_eq!(
      String::from_utf8_lossy(&lost.stderr),
      "ts-quality: toolu not found; TypeScript checks skipped\n"
    );
  }
  stub(
    &shelf.join("toolu"),
    r#"[ "$1" = --hook-protocol ] && echo 1 || echo "native:$*""#,
  );
  let ran = bridge(&script, &path, &WRITE_TS);
  assert_eq!(
    String::from_utf8_lossy(&ran.stdout),
    "native:ts-quality hook post-tools --event PostToolUse\n"
  );
}

#[test]
fn codex_without_the_core_is_warned_and_other_hosts_are_silent() {
  let dir = tempfile::tempdir().unwrap();
  let stub = dir.path().join("codex");
  let plugins = r#"{"installed":[{"pluginId":"ts-quality@toolu"}]}"#;
  std::fs::write(&stub, format!("#!/bin/sh\nprintf '%s\\n' '{plugins}'\n")).unwrap();
  std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
  let search = format!("{}:/usr/bin:/bin", dir.path().display());
  let codex = Env::from_pairs([
    ("PATH", search.as_str()),
    ("CODEX_HOME", "/c"),
    ("PLUGIN_ROOT", "/p"),
  ]);
  let warned = check_toolu(&codex);
  assert_eq!(warned.exit, Exit::Success);
  let text = warned.stdout.unwrap_or_default();
  assert!(
    text.contains("toolu@toolu") && !text.ends_with('\n'),
    "{text}"
  );
  let claude = Env::from_pairs([("CLAUDE_PLUGIN_ROOT", "/p")]);
  assert_eq!(check_toolu(&claude).stdout, None);
}
