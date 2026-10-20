//! Muse quality registration and post-edit enforcement through the real CLI.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn run(project: &Path, config: &Path, args: &[&str], payload: &str, report: &Path) -> Output {
  let mut child = Command::new(TOOLU)
    .args(args)
    .current_dir(project)
    .env("HOME", project)
    .env("TOOLU_HOST_OVERRIDE", "muse")
    .env("MUSE_CONFIG_DIR", config)
    .env("TOOLU_PROJECT_DIR", project)
    .env("TOOLU_STARTUP_REPORT", report)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(payload.as_bytes())
    .unwrap();
  child.wait_with_output().unwrap()
}

fn plugin_root(name: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

#[test]
fn muse_registers_three_rules_and_enforces_a_rust_edit() {
  let temp = tempfile::tempdir().unwrap();
  let project = temp.path().join("project");
  let config = temp.path().join("muse-config");
  let report = temp.path().join("startup.jsonl");
  fs::create_dir_all(project.join("src")).unwrap();
  fs::create_dir_all(&config).unwrap();
  fs::write(&report, "").unwrap();
  fs::write(
    project.join("Cargo.toml"),
    "[package]\nname=\"demo\"\nversion=\"0.1.0\"\n",
  )
  .unwrap();
  let git = Command::new("git")
    .args(["init", "-q"])
    .current_dir(&project)
    .status()
    .unwrap();
  assert!(git.success());
  let post = config.join("toolu/post-tools.d");
  let pre = config.join("toolu/pre-tools.d");
  fs::create_dir_all(&post).unwrap();
  fs::create_dir_all(&pre).unwrap();
  fs::write(post.join("rust-quality@toolu__rust-quality.js"), "old").unwrap();
  fs::write(pre.join("rust-quality@toolu__old.sh"), "old").unwrap();

  for plugin in ["ts-quality", "python-quality", "rust-quality"] {
    let root = plugin_root(plugin).display().to_string();
    let args = [
      plugin,
      "hook",
      "register",
      "--event",
      "SessionStart",
      "--plugin-root",
      &root,
    ];
    let registered = run(&project, &config, &args, "{}", &report);
    assert_eq!(
      registered.status.code(),
      Some(0),
      "{plugin}: {registered:?}"
    );
    let manifest = post.join(format!("{plugin}@toolu__{plugin}.json"));
    assert!(manifest.is_file(), "{plugin}");
    let bytes = fs::read(&manifest).unwrap();
    let modified = fs::metadata(&manifest).unwrap().modified().unwrap();
    let again = run(&project, &config, &args, "{}", &report);
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert_eq!(fs::read(&manifest).unwrap(), bytes);
    assert_eq!(
      fs::metadata(&manifest).unwrap().modified().unwrap(),
      modified
    );
    let check = [
      plugin,
      "hook",
      "check-toolu",
      "--event",
      "SessionStart",
      "--plugin-root",
      &root,
    ];
    let checked = run(&project, &config, &check, "{}", &report);
    assert_eq!((checked.status.code(), checked.stdout.len()), (Some(0), 0));
  }
  assert!(!post.join("rust-quality@toolu__rust-quality.js").exists());
  assert!(!pre.join("rust-quality@toolu__old.sh").exists());
  let lines = fs::read_to_string(&report).unwrap();
  assert_eq!(lines.lines().count(), 6);
  assert!(lines.contains(&post.display().to_string()));

  let bad = "//! demo\n#[allow(dead_code)]\nfn bad() {}\n";
  let file = project.join("src/lib.rs");
  fs::write(&file, bad).unwrap();
  let payload = json!({"session_id":"s","cwd":project,"tool_name":"write_file",
    "tool_input":{"file_path":file,"content":bad},"tool_response":{}});
  let edited = run(
    &project,
    &config,
    &["hook", "post-tools", "--event", "PostToolUse"],
    &payload.to_string(),
    &report,
  );
  assert_eq!(edited.status.code(), Some(0), "{edited:?}");
  let output: Value = serde_json::from_slice(&edited.stdout).unwrap();
  assert!(
    output["systemMessage"]
      .as_str()
      .unwrap()
      .contains("Forbidden lint suppression")
  );
  let gate = fs::read_to_string(project.join(".agents/tmp/quality-gate-status.json")).unwrap();
  assert!(gate.contains("\"status\": \"failing\""), "{gate}");
  let commit = json!({"session_id":"s","cwd":project,"tool_name":"bash",
    "tool_input":{"command":"git commit -m test"}});
  let before = run(
    &project,
    &config,
    &["hook", "pre-tools", "--event", "PreToolUse"],
    &commit.to_string(),
    &report,
  );
  assert_eq!(before.status.code(), Some(0), "{before:?}");
  let decision: Value = serde_json::from_slice(&before.stdout).unwrap();
  assert_eq!(decision["decision"], "block", "{decision}");
}
