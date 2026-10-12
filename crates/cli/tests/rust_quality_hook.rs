//! rust-quality's native `SessionStart` register and `check-toolu` in a real
//! config dir, the real `toolu hook post-tools` on Rust and non-Rust edits, and
//! `toolu rust-quality check` on real repositories.

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
const POST: &str = ".claude/toolu/post-tools.d";
const MANIFEST: &str = "rust-quality@toolu__rust-quality.json";
const BRIDGE: &str = "rust-quality@toolu__rust-quality.sh";
const CLEAR: &[&str] = &[
  "TOOLU_CONFIG_DIR",
  "TOOLU_USER_CONFIG_DIR",
  "TOOLU_SETTINGS_DIR",
  "CLAUDE_CONFIG_DIR",
  "CLAUDE_PROJECT_DIR",
  "CLAUDE_PLUGIN_ROOT",
  "CLAUDE_PLUGINS_REGISTRY",
  "CODEX_HOME",
  "PLUGIN_ROOT",
  "TOOLU_IMPL",
  "TOOLU_STARTUP_REPORT",
];

/// A home holding a Cargo project in git, rust-quality installed or not.
struct Sandbox {
  _dir: tempfile::TempDir,
  home: PathBuf,
  project: PathBuf,
}

fn git(project: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git")
    .args(args)
    .current_dir(project)
    .env("GIT_AUTHOR_NAME", "t")
    .env("GIT_AUTHOR_EMAIL", "t@t")
    .env("GIT_COMMITTER_NAME", "t")
    .env("GIT_COMMITTER_EMAIL", "t@t")
    .status()?;
  if !status.success() {
    return Err(format!("git {args:?} failed").into());
  }
  Ok(())
}

fn sandbox(installed: bool) -> Res<Sandbox> {
  let dir = tempfile::tempdir()?;
  let home = dir.path().join("home");
  let project = dir.path().join("project");
  fs::create_dir_all(home.join(".claude/plugins"))?;
  fs::create_dir_all(project.join("src"))?;
  let specs = if installed {
    json!({"toolu@toolu": [{"scope": "user"}], "rust-quality@toolu": [{"scope": "user"}]})
  } else {
    json!({"toolu@toolu": [{"scope": "user"}]})
  };
  fs::write(
    home.join(".claude/plugins/installed_plugins.json"),
    json!({"version": 2, "plugins": specs}).to_string(),
  )?;
  fs::write(
    project.join("Cargo.toml"),
    "[package]\nname = \"x\"\nversion = \"0.1.0\"\n",
  )?;
  git(&project, &["init", "-q"])?;
  git(&project, &["add", "."])?;
  git(&project, &["commit", "-q", "-m", "cargo project"])?;
  Ok(Sandbox {
    _dir: dir,
    home,
    project,
  })
}

fn run(sb: &Sandbox, args: &[&str], env: &[(&str, &str)], stdin: &str) -> Res<Output> {
  let mut command = Command::new(TOOLU);
  command
    .args(args)
    .env("HOME", &sb.home)
    .env("PATH", std::env::var("PATH")?)
    .env("TOOLU_HOST_OVERRIDE", "claude")
    .current_dir(&sb.project)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  for key in CLEAR {
    command.env_remove(key);
  }
  command.envs(env.iter().copied());
  let mut child = command.spawn()?;
  child
    .stdin
    .take()
    .ok_or("no stdin")?
    .write_all(stdin.as_bytes())?;
  Ok(child.wait_with_output()?)
}

fn session_start(sb: &Sandbox, name: &str, env: &[(&str, &str)]) -> Res<Output> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/rust-quality");
  let root = root.display().to_string();
  let args = [
    "rust-quality",
    "hook",
    name,
    "--event",
    "SessionStart",
    "--plugin-root",
    &root,
  ];
  run(sb, &args, env, "{}")
}

/// `toolu hook post-tools` after a `Write` of `rel` holding `body`.
fn write(sb: &Sandbox, rel: &str, body: &str, env: &[(&str, &str)]) -> Res<Output> {
  let file = sb.project.join(rel);
  fs::create_dir_all(file.parent().ok_or("no parent")?)?;
  fs::write(&file, body)?;
  let payload = json!({"session_id": "s", "cwd": sb.project, "tool_name": "Write",
    "tool_input": {"file_path": file, "content": body}, "tool_response": {}});
  let args = ["hook", "post-tools", "--event", "PostToolUse"];
  run(sb, &args, env, &payload.to_string())
}

fn listing(dir: &Path) -> Res<Vec<String>> {
  let mut names = fs::read_dir(dir)?
    .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
    .collect::<Res<Vec<_>>>()?;
  names.sort();
  Ok(names)
}

fn registry_check(sb: &Sandbox) -> Res<Value> {
  let doctor = run(sb, &["--json", "doctor"], &[], "")?;
  let document: Value = serde_json::from_slice(&doctor.stdout)?;
  let checks = document["checks"].as_array().ok_or("no checks")?;
  let registry = checks.iter().find(|check| check["id"] == "registry");
  Ok(registry.ok_or("no registry check")?.clone())
}

#[test]
fn register_migrates_then_the_native_hook_reports_and_doctor_is_clean() {
  let sb = sandbox(true).unwrap();
  let post = sb.home.join(POST);
  fs::create_dir_all(&post).unwrap();
  fs::write(
    post.join("rust-quality@toolu__rust-quality.js"),
    "export default {};\n",
  )
  .unwrap();
  let before = registry_check(&sb).unwrap();
  assert!(before.to_string().contains("Bun bridge"), "{before}");
  let first = session_start(&sb, "register", &[]).unwrap();
  assert!(first.status.success(), "{first:?}");
  let notice: Value = serde_json::from_slice(&first.stdout).unwrap();
  assert_eq!(
    notice["systemMessage"],
    "rust-quality's Bun module was replaced by its native rule in toolu."
  );
  assert_eq!(listing(&post).unwrap(), [MANIFEST, BRIDGE]);
  let manifest = fs::read_to_string(post.join(MANIFEST)).unwrap();
  let again = session_start(&sb, "register", &[]).unwrap();
  assert_eq!((again.status.code(), again.stdout.len()), (Some(0), 0));
  assert_eq!(fs::read_to_string(post.join(MANIFEST)).unwrap(), manifest);

  let bad = "//! X.\n#[allow(dead_code)]\nfn a() {}\n";
  let fired = write(&sb, "src/lib.rs", bad, &[]).unwrap();
  assert!(fired.status.success(), "{fired:?}");
  let stdout = String::from_utf8(fired.stdout).unwrap();
  assert!(stdout.contains("Forbidden lint suppression"), "{stdout}");
  let after = registry_check(&sb).unwrap();
  assert_eq!(after["status"], "ok", "{after}");
}

#[test]
fn an_uninstalled_plugin_stays_silent() {
  let sb = sandbox(false).unwrap();
  assert!(
    session_start(&sb, "register", &[])
      .unwrap()
      .status
      .success()
  );
  let output = write(&sb, "src/lib.rs", "#[allow(x)]\nfn a() {}\n", &[]).unwrap();
  assert!(output.status.success(), "{output:?}");
  assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
}

#[test]
fn non_rust_edits_spawn_no_tool() {
  let sb = sandbox(true).unwrap();
  assert!(
    session_start(&sb, "register", &[])
      .unwrap()
      .status
      .success()
  );
  let stubs = sb.home.join("stubs");
  fs::create_dir_all(&stubs).unwrap();
  let log = sb.home.join("spawned.log");
  for tool in ["ast-grep", "sg", "cargo", "bun", "node"] {
    let stub = stubs.join(tool);
    let body = format!(
      "#!/bin/sh\necho \"{tool} $*\" >> '{}'\necho '[]'\n",
      log.display()
    );
    fs::write(&stub, body).unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
  }
  let path = format!("{}:/usr/bin:/bin", stubs.display());
  let env = [("PATH", path.as_str())];
  for (rel, body) in [
    ("src/a.ts", "export const a = 1;\n"),
    ("README.md", "# x\n"),
  ] {
    let output = write(&sb, rel, body, &env).unwrap();
    assert!(output.status.success(), "{rel}: {output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "", "{rel}");
  }
  assert!(!log.exists(), "{}", fs::read_to_string(&log).unwrap());
  let output = write(&sb, "src/lib.rs", "//! A.\n", &env).unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(
    fs::read_to_string(&log)
      .unwrap()
      .starts_with("ast-grep scan"),
    "a .rs edit reaches ast-grep"
  );
}

#[test]
fn check_toolu_warns_on_codex_without_the_core() {
  let sb = sandbox(true).unwrap();
  let bin = sb.home.join("bin");
  fs::create_dir_all(&bin).unwrap();
  let codex_home = sb.home.join(".codex").display().to_string();
  let path = format!("{}:/usr/bin:/bin", bin.display());
  let mut outputs = Vec::new();
  for plugins in [
    r#"[{"pluginId":"rust-quality@toolu"}]"#,
    r#"[{"pluginId":"rust-quality@toolu"},{"pluginId":"toolu@toolu"}]"#,
  ] {
    let list = format!("{{\"installed\":{plugins}}}");
    fs::write(
      bin.join("codex"),
      format!("#!/bin/sh\nprintf '%s\\n' '{list}'\n"),
    )
    .unwrap();
    fs::set_permissions(bin.join("codex"), fs::Permissions::from_mode(0o755)).unwrap();
    let env = [
      ("PATH", path.as_str()),
      ("CODEX_HOME", codex_home.as_str()),
      ("PLUGIN_ROOT", "/plugins/rust-quality"),
      ("TOOLU_HOST_OVERRIDE", "codex"),
    ];
    outputs.push(session_start(&sb, "check-toolu", &env).unwrap());
  }
  assert_eq!(outputs[0].status.code(), Some(0), "{:?}", outputs[0]);
  let warned = String::from_utf8(outputs[0].stdout.clone()).unwrap();
  assert!(warned.contains("toolu@toolu"), "{:?}", outputs[0]);
  assert_eq!(
    (outputs[1].status.code(), outputs[1].stdout.len()),
    (Some(0), 0)
  );
  let claude = session_start(&sb, "check-toolu", &[]).unwrap();
  assert_eq!((claude.status.code(), claude.stdout.len()), (Some(0), 0));
}

#[test]
fn check_runs_the_blocking_rules_over_a_repository() {
  let sb = sandbox(true).unwrap();
  let over: String = (0..320)
    .map(|at| format!("const A{at}: u32 = {at};\n"))
    .collect();
  fs::create_dir_all(sb.project.join("crates/demo/src")).unwrap();
  fs::write(sb.project.join("crates/demo/src/over.rs"), &over).unwrap();
  let fits = run(&sb, &["rust-quality", "check", "crates"], &[], "").unwrap();
  assert_eq!(fits.status.code(), Some(0), "{fits:?}");
  assert_eq!(
    String::from_utf8(fits.stderr).unwrap(),
    "rust-quality: 1 files, ok\n"
  );
  fs::create_dir_all(sb.project.join(".claude")).unwrap();
  let capped = json!({"lang": {"rust": {"maxFileLines": 300}}}).to_string();
  fs::write(sb.project.join(".claude/toolu.config.json"), capped).unwrap();
  let over_limit = run(&sb, &["rust-quality", "check", "crates"], &[], "").unwrap();
  assert_eq!(over_limit.status.code(), Some(1), "{over_limit:?}");
  let stdout = String::from_utf8(over_limit.stdout).unwrap();
  assert!(
    stdout.starts_with("File exceeds 300-line limit: crates/demo/src/over.rs (320 code lines"),
    "{stdout}"
  );
  let json = run(&sb, &["--json", "rust-quality", "check"], &[], "").unwrap();
  let doc: Value = serde_json::from_slice(&json.stdout).unwrap();
  assert_eq!(
    (doc["files"].as_u64(), doc["findings"][0]["path"].as_str()),
    (Some(1), Some("crates/demo/src/over.rs"))
  );
  let bare = run(
    &sb,
    &["rust-quality", "check"],
    &[("PATH", "/usr/bin:/bin")],
    "",
  )
  .unwrap();
  assert_eq!(bare.status.code(), Some(69), "{bare:?}");
}
