//! rust-quality's native `SessionStart` retirement and `check-toolu` in a real
//! config dir, the real `toolu hook post-tools` proving no edit runs a rule, and
//! `toolu rust-quality check` on real repositories.

#[path = "helpers/feed.rs"]
mod feed;
use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
const POST: &str = ".claude/toolu/post-tools.d";
const MANIFEST: &str = "rust-quality@toolu__rust-quality.json";
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
  feed::feed(child.stdin.take(), stdin.as_bytes())?;
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
  let checks = document
    .get("checks")
    .and_then(Value::as_array)
    .ok_or("no checks")?;
  let registry = checks
    .iter()
    .find(|check| check.get("id").and_then(Value::as_str) == Some("registry"));
  Ok(registry.ok_or("no registry check")?.clone())
}

#[test]
fn forget_retires_the_registration_and_doctor_is_clean() {
  let sb = sandbox(true).unwrap();
  let post = sb.home.join(POST);
  fs::create_dir_all(&post).unwrap();
  fs::write(post.join(MANIFEST), "{\"version\":1}\n").unwrap();
  fs::write(
    post.join("rust-quality@toolu__rust-quality.js"),
    "export default {};\n",
  )
  .unwrap();
  let first = session_start(&sb, "forget", &[]).unwrap();
  assert_eq!((first.status.code(), first.stdout.len()), (Some(0), 0));
  assert_eq!(listing(&post).unwrap(), Vec::<String>::new());
  let again = session_start(&sb, "forget", &[]).unwrap();
  assert_eq!((again.status.code(), again.stdout.len()), (Some(0), 0));

  let bad = concat!("//! X.\n#[", "allow(dead_code)]\nfn a() {}\n");
  let fired = write(&sb, "src/lib.rs", bad, &[]).unwrap();
  assert!(fired.status.success(), "{fired:?}");
  assert_eq!(String::from_utf8(fired.stdout).unwrap(), "");
  assert!(
    !sb
      .project
      .join(".claude/tmp/quality-gate-status.json")
      .exists(),
    "an edit still recorded a gate state"
  );
  let after = registry_check(&sb).unwrap();
  assert_eq!(after["status"], "ok", "{after}");
}

#[test]
fn an_uninstalled_plugin_stays_silent() {
  let sb = sandbox(false).unwrap();
  let retired = session_start(&sb, "forget", &[]).unwrap();
  assert!(retired.status.success(), "{retired:?}");
  let bad = concat!("#[", "allow(x)]\nfn a() {}\n");
  let output = write(&sb, "src/lib.rs", bad, &[]).unwrap();
  assert!(output.status.success(), "{output:?}");
  assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
}

#[test]
fn no_edit_spawns_a_tool() {
  let sb = sandbox(true).unwrap();
  let retired = session_start(&sb, "forget", &[]).unwrap();
  assert!(retired.status.success(), "{retired:?}");
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
    ("src/lib.rs", "//! A.\n"),
  ] {
    let output = write(&sb, rel, body, &env).unwrap();
    assert!(output.status.success(), "{rel}: {output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "", "{rel}");
  }
  assert!(!log.exists(), "{}", fs::read_to_string(&log).unwrap());
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
  let over = (0..320)
    .map(|at| format!("const A{at}: u32 = {at};\n"))
    .collect::<Vec<_>>()
    .concat();
  fs::create_dir_all(sb.project.join("crates/demo/src")).unwrap();
  fs::write(sb.project.join("crates/demo/src/over.rs"), &over).unwrap();
  let fits = run(&sb, &["rust-quality", "check", "crates"], &[], "").unwrap();
  assert_eq!(fits.status.code(), Some(0), "{fits:?}");
  assert_eq!(
    String::from_utf8(fits.stderr).unwrap(),
    "rust-quality: 1 files, ok\n"
  );
  fs::write(
    sb.project.join("crates/demo/src/bad.rs"),
    concat!("//! B.\n#[", "allow(dead_code)]\nfn b() {}\n"),
  )
  .unwrap();
  let blocked = run(&sb, &["rust-quality", "check", "crates"], &[], "").unwrap();
  assert_eq!(blocked.status.code(), Some(1), "{blocked:?}");
  let stdout = String::from_utf8(blocked.stdout).unwrap();
  assert!(stdout.contains("Forbidden lint suppression"), "{stdout}");
  let json = run(&sb, &["--json", "rust-quality", "check"], &[], "").unwrap();
  let doc: Value = serde_json::from_slice(&json.stdout).unwrap();
  assert_eq!(
    doc["findings"][0]["path"].as_str(),
    Some("crates/demo/src/bad.rs")
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
