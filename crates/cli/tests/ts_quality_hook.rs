//! ts-quality's native `SessionStart` register in a real config dir, then the
//! real `toolu hook post-tools` on TypeScript and non-TypeScript edits.

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
const MANIFEST: &str = "ts-quality@toolu__ts-quality.json";
const CLEAR: &[&str] = &[
  "TOOLU_CONFIG_DIR",
  "CLAUDE_CONFIG_DIR",
  "CLAUDE_PROJECT_DIR",
  "CLAUDE_PLUGINS_REGISTRY",
  "CODEX_HOME",
  "TOOLU_IMPL",
  "TOOLU_STARTUP_REPORT",
];
const TOOLS: &[&str] = &[
  "ast-grep", "sg", "npm", "npx", "bunx", "bun", "node", "tsc", "biome", "eslint", "oxlint",
  "jscpd",
];

/// A home holding a git project with a TypeScript config, ts-quality installed or not.
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
    json!({"toolu@toolu": [{"scope": "user"}], "ts-quality@toolu": [{"scope": "user"}]})
  } else {
    json!({"toolu@toolu": [{"scope": "user"}]})
  };
  fs::write(
    home.join(".claude/plugins/installed_plugins.json"),
    json!({"version": 2, "plugins": specs}).to_string(),
  )?;
  fs::write(project.join("tsconfig.json"), "{}\n")?;
  fs::write(project.join("package.json"), "{\"name\":\"x\"}\n")?;
  fs::write(project.join("package-lock.json"), "{}\n")?;
  git(&project, &["init", "-q"])?;
  git(&project, &["add", "."])?;
  git(&project, &["commit", "-q", "-m", "ts project"])?;
  Ok(Sandbox {
    _dir: dir,
    home,
    project,
  })
}

fn toolu(sb: &Sandbox, args: &[&str], path: &str, stdin: &str) -> Res<Output> {
  let mut command = Command::new(TOOLU);
  command
    .args(args)
    .env("HOME", &sb.home)
    .env("PATH", path)
    .env("TOOLU_HOST_OVERRIDE", "claude")
    .current_dir(&sb.project)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  for key in CLEAR {
    command.env_remove(key);
  }
  let mut child = command.spawn()?;
  feed::feed(child.stdin.take(), stdin.as_bytes())?;
  Ok(child.wait_with_output()?)
}

fn register(sb: &Sandbox) -> Res<Output> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/ts-quality");
  let root = root.display().to_string();
  let args = [
    "ts-quality",
    "hook",
    "register",
    "--event",
    "SessionStart",
    "--plugin-root",
    &root,
  ];
  toolu(sb, &args, &std::env::var("PATH")?, "{}")
}

/// `toolu hook post-tools` after a `Write` of `rel` holding `body`.
fn write(sb: &Sandbox, rel: &str, body: &str, path: &str) -> Res<Output> {
  let file = sb.project.join(rel);
  fs::write(&file, body)?;
  let payload = json!({"session_id": "s", "cwd": sb.project, "tool_name": "Write",
    "tool_input": {"file_path": file, "content": body}, "tool_response": {}});
  let args = ["hook", "post-tools", "--event", "PostToolUse"];
  toolu(sb, &args, path, &payload.to_string())
}

fn listing(dir: &Path) -> Res<Vec<String>> {
  let mut names = fs::read_dir(dir)?
    .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
    .collect::<Res<Vec<_>>>()?;
  names.sort();
  Ok(names)
}

fn snapshot(dir: &Path) -> Res<Vec<(String, std::time::SystemTime)>> {
  [MANIFEST]
    .iter()
    .map(|name| {
      let path = dir.join(name);
      Ok((fs::read_to_string(&path)?, fs::metadata(&path)?.modified()?))
    })
    .collect()
}

#[test]
fn register_migrates_then_the_native_hook_reports_and_doctor_is_clean() {
  let sb = sandbox(true).unwrap();
  let post = sb.home.join(POST);
  fs::create_dir_all(&post).unwrap();
  fs::write(post.join("ts-quality@toolu__ts-quality.js"), "old\n").unwrap();
  fs::write(post.join("ts-quality@toolu__old-concern.sh"), "old\n").unwrap();
  let first = register(&sb).unwrap();
  assert!(first.status.success(), "{first:?}");
  let notice: Value = serde_json::from_slice(&first.stdout).unwrap();
  assert_eq!(
    notice["systemMessage"],
    "ts-quality's JavaScript module was replaced by its native rule in toolu."
  );
  assert_eq!(listing(&post).unwrap(), [MANIFEST]);
  let before = snapshot(&post).unwrap();
  let again = register(&sb).unwrap();
  assert_eq!((again.status.code(), again.stdout.len()), (Some(0), 0));
  assert_eq!(snapshot(&post).unwrap(), before);

  let fired = write(
    &sb,
    "src/bad.ts",
    "console.log(\"x\");\n",
    &std::env::var("PATH").unwrap(),
  )
  .unwrap();
  assert!(fired.status.success(), "{fired:?}");
  let stdout = String::from_utf8(fired.stdout.clone()).unwrap();
  assert!(stdout.contains("Forbidden console.log"), "{fired:?}");

  let doctor = toolu(
    &sb,
    &["--json", "doctor"],
    &std::env::var("PATH").unwrap(),
    "",
  )
  .unwrap();
  let document: Value = serde_json::from_slice(&doctor.stdout).unwrap();
  let registry = document["checks"]
    .as_array()
    .and_then(|checks| checks.iter().find(|check| check["id"] == "registry"))
    .expect("no registry check");
  assert_eq!(registry["status"], "ok", "{registry}");
}

#[test]
fn an_uninstalled_plugin_stays_silent() {
  let sb = sandbox(false).unwrap();
  assert!(register(&sb).unwrap().status.success());
  let output = write(
    &sb,
    "src/bad.ts",
    "console.log(\"x\");\n",
    &std::env::var("PATH").unwrap(),
  )
  .unwrap();
  assert!(output.status.success(), "{output:?}");
  assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
}

#[test]
fn non_typescript_edits_spawn_no_tool() {
  let sb = sandbox(true).unwrap();
  assert!(register(&sb).unwrap().status.success());
  let stubs = sb.home.join("stubs");
  fs::create_dir_all(&stubs).unwrap();
  let log = sb.home.join("spawned.log");
  for tool in TOOLS {
    let stub = stubs.join(tool);
    fs::write(
      &stub,
      format!("#!/bin/sh\necho \"{tool} $*\" >> '{}'\n", log.display()),
    )
    .unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
  }
  let path = format!("{}:/usr/bin:/bin", stubs.display());
  for (rel, body) in [("src/a.py", "print(1)\n"), ("README.md", "# x\n")] {
    let output = write(&sb, rel, body, &path).unwrap();
    assert!(output.status.success(), "{rel}: {output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "", "{rel}");
  }
  assert!(!log.exists(), "{}", fs::read_to_string(&log).unwrap());
  let output = write(&sb, "src/b.ts", "export const b = 1;\n", &path).unwrap();
  assert!(output.status.success(), "{output:?}");
  assert!(log.exists(), "a .ts edit reaches its tools");
}
