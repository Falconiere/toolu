//! `toolu setup agents` matches `bun setup.ts` aside from the program name.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
const SCRIPT: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../plugins/toolu/skills/setup/scripts/setup.ts"
);
const TEMPLATES: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../plugins/toolu/assets/agents"
);
const NAMES: [&str; 5] = [
  "quick-task",
  "deep-explore",
  "research-agent",
  "implementer",
  "architect",
];

#[derive(Clone, Copy)]
enum Home {
  Codex,
  Fallback,
  Unset,
}

struct Side {
  root: PathBuf,
}

struct Out {
  code: i32,
  stdout: String,
  stderr: String,
}

fn sandbox() -> Result<Side, String> {
  let root = tempfile::tempdir()
    .map_err(|_err| "temp".to_owned())?
    .into_path();
  Ok(Side { root })
}

fn capture(output: Output) -> Result<Out, String> {
  Ok(Out {
    code: output.status.code().ok_or("signal")?,
    stdout: String::from_utf8(output.stdout).map_err(|_err| "stdout".to_owned())?,
    stderr: String::from_utf8(output.stderr).map_err(|_err| "stderr".to_owned())?,
  })
}

fn clear(command: &mut Command) {
  for key in [
    "TOOLU_HOST_OVERRIDE",
    "TOOLU_AGENT_TEMPLATE_DIR",
    "TOOLU_TIMESTAMP",
    "TOOLU_PROJECT_DIR",
  ] {
    command.env_remove(key);
  }
}

fn place_home(command: &mut Command, side: &Side, home: Home) {
  match home {
    Home::Codex => {
      command.env("HOME", &side.root);
      command.env("CODEX_HOME", side.root.join("codex"));
    }
    Home::Fallback => {
      command.env("HOME", &side.root);
      command.env_remove("CODEX_HOME");
    }
    Home::Unset => {
      command.env_remove("HOME");
      command.env_remove("CODEX_HOME");
    }
  }
}

fn script(side: &Side, home: Home, extra: &[(&str, &str)], args: &[&str]) -> Result<Out, String> {
  let mut command = Command::new("bun");
  command.arg(SCRIPT).args(args);
  clear(&mut command);
  place_home(&mut command, side, home);
  for (key, value) in extra {
    command.env(key, value);
  }
  capture(command.output().map_err(|_err| "bun".to_owned())?)
}

fn toolu(side: &Side, home: Home, extra: &[(&str, &str)], args: &[&str]) -> Result<Out, String> {
  let mut command = Command::new(TOOLU);
  command.arg("setup").arg("agents").args(args);
  clear(&mut command);
  place_home(&mut command, side, home);
  for (key, value) in extra {
    command.env(key, value);
  }
  capture(command.output().map_err(|_err| "toolu".to_owned())?)
}

fn norm(text: &str, root: &Path) -> String {
  text
    .replace(&root.display().to_string(), "ROOT")
    .replace("setup.ts", "toolu setup agents")
}

fn compare(left: &Side, right: &Side, bun: &Out, native: &Out) -> Result<(), String> {
  if native.code != bun.code {
    return Err(format!(
      "exit {} != {}\n{}",
      native.code, bun.code, native.stderr
    ));
  }
  let stdout = norm(&native.stdout, &right.root);
  let script_out = norm(&bun.stdout, &left.root);
  if stdout != script_out {
    return Err(format!("stdout\n{stdout}\n---\n{script_out}"));
  }
  let stderr = norm(&native.stderr, &right.root);
  let script_err = norm(&bun.stderr, &left.root);
  if stderr != script_err {
    return Err(format!("stderr\n{stderr}\n---\n{script_err}"));
  }
  Ok(())
}

fn once(home: Home, extra: &[(&str, &str)], args: &[&str]) -> Result<(), String> {
  let left = sandbox()?;
  let right = sandbox()?;
  let bun = script(&left, home, extra, args)?;
  let native = toolu(&right, home, extra, args)?;
  compare(&left, &right, &bun, &native)
}

fn codex_file(side: &Side, name: &str) -> PathBuf {
  side
    .root
    .join("codex")
    .join("agents")
    .join(format!("{name}.toml"))
}

fn write_personal(side: &Side) -> Result<(), String> {
  let agents = side.root.join("codex").join("agents");
  fs::create_dir_all(&agents).map_err(|_err| "mkdir".to_owned())?;
  fs::write(agents.join("quick-task.toml"), "name = \"personal\"\n")
    .map_err(|_err| "write".to_owned())
}

fn copy_bad_templates(side: &Side) -> Result<PathBuf, String> {
  let dir = side.root.join("templates");
  fs::create_dir_all(&dir).map_err(|_err| "mkdir".to_owned())?;
  for name in NAMES {
    fs::copy(
      Path::new(TEMPLATES).join(format!("{name}.toml")),
      dir.join(format!("{name}.toml")),
    )
    .map_err(|_err| "copy".to_owned())?;
  }
  let mut file = fs::OpenOptions::new()
    .append(true)
    .open(dir.join("architect.toml"))
    .map_err(|_err| "open".to_owned())?;
  writeln!(file, "invalid = [").map_err(|_err| "append".to_owned())?;
  Ok(dir)
}

#[test]
fn preview_install_and_refusal_match_the_script() {
  once(Home::Fallback, &[], &["preview"]).unwrap();
  once(Home::Codex, &[], &["install"]).unwrap();
  once(Home::Codex, &[], &["install", "--now"]).unwrap();
  once(Home::Codex, &[], &["deploy"]).unwrap();
  once(Home::Unset, &[], &["preview"]).unwrap();
  once(
    Home::Codex,
    &[("TOOLU_HOST_OVERRIDE", "opencode")],
    &["install"],
  )
  .unwrap();
}

#[test]
fn a_second_install_and_a_managed_update_match() {
  let left = sandbox().unwrap();
  let right = sandbox().unwrap();
  let stamp = [("TOOLU_TIMESTAMP", "20260813T190000Z")];
  let bun = script(&left, Home::Codex, &[], &["install"]).unwrap();
  let native = toolu(&right, Home::Codex, &[], &["install"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let again = script(&left, Home::Codex, &[], &["install"]).unwrap();
  let again_native = toolu(&right, Home::Codex, &[], &["install"]).unwrap();
  compare(&left, &right, &again, &again_native).unwrap();
  let profile = "model_reasoning_effort = \"medium\"";
  let low = "model_reasoning_effort = \"low\"";
  for side in [&left, &right] {
    let path = codex_file(side, "quick-task");
    let text = fs::read_to_string(&path).unwrap().replace(profile, low);
    fs::write(&path, text).unwrap();
  }
  let bun = script(&left, Home::Codex, &stamp, &["install"]).unwrap();
  let native = toolu(&right, Home::Codex, &stamp, &["install"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let backup = "codex/agents/.toolu-backups/20260813T190000Z/quick-task.toml";
  assert_eq!(
    fs::read(left.root.join(backup)).unwrap(),
    fs::read(right.root.join(backup)).unwrap()
  );
}

#[test]
fn conflict_remove_and_invalid_templates_match() {
  let left = sandbox().unwrap();
  let right = sandbox().unwrap();
  write_personal(&left).unwrap();
  write_personal(&right).unwrap();
  let preview = script(&left, Home::Codex, &[], &["preview"]).unwrap();
  let preview_native = toolu(&right, Home::Codex, &[], &["preview"]).unwrap();
  compare(&left, &right, &preview, &preview_native).unwrap();
  let bun = script(&left, Home::Codex, &[], &["install"]).unwrap();
  let native = toolu(&right, Home::Codex, &[], &["install"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let stamp = [("TOOLU_TIMESTAMP", "20260813T190100Z")];
  let bun = script(&left, Home::Codex, &stamp, &["install", "--force"]).unwrap();
  let native = toolu(&right, Home::Codex, &stamp, &["install", "--force"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let refused = script(&left, Home::Codex, &[], &["remove"]).unwrap();
  let refused_native = toolu(&right, Home::Codex, &[], &["remove"]).unwrap();
  compare(&left, &right, &refused, &refused_native).unwrap();
  let gone_stamp = [("TOOLU_TIMESTAMP", "20260813T190200Z")];
  let gone = script(&left, Home::Codex, &gone_stamp, &["remove", "--yes"]).unwrap();
  let gone_native = toolu(&right, Home::Codex, &gone_stamp, &["remove", "--yes"]).unwrap();
  compare(&left, &right, &gone, &gone_native).unwrap();
  let bad_left = copy_bad_templates(&left).unwrap();
  let bad_right = copy_bad_templates(&right).unwrap();
  let left_dir = bad_left.display().to_string();
  let right_dir = bad_right.display().to_string();
  let bun = script(
    &left,
    Home::Codex,
    &[("TOOLU_AGENT_TEMPLATE_DIR", &left_dir)],
    &["install"],
  )
  .unwrap();
  let native = toolu(
    &right,
    Home::Codex,
    &[("TOOLU_AGENT_TEMPLATE_DIR", &right_dir)],
    &["install"],
  )
  .unwrap();
  compare(&left, &right, &bun, &native).unwrap();
}

#[test]
fn forced_remove_bad_timestamp_and_existing_backup_match() {
  let left = sandbox().unwrap();
  let right = sandbox().unwrap();
  write_personal(&left).unwrap();
  write_personal(&right).unwrap();
  let force = [("TOOLU_TIMESTAMP", "20260813T190300Z")];
  let bun = script(&left, Home::Codex, &force, &["remove", "--yes", "--force"]).unwrap();
  let native = toolu(&right, Home::Codex, &force, &["remove", "--yes", "--force"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let bun = script(&left, Home::Codex, &[], &["install"]).unwrap();
  let native = toolu(&right, Home::Codex, &[], &["install"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let bad = [("TOOLU_TIMESTAMP", "../x")];
  let bun = script(&left, Home::Codex, &bad, &["remove", "--yes"]).unwrap();
  let native = toolu(&right, Home::Codex, &bad, &["remove", "--yes"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
  let from = "model_reasoning_effort = \"medium\"";
  let to = "model_reasoning_effort = \"low\"";
  for side in [&left, &right] {
    let path = codex_file(side, "quick-task");
    let text = fs::read_to_string(&path).unwrap().replace(from, to);
    fs::write(&path, text).unwrap();
    let dir = side
      .root
      .join("codex/agents/.toolu-backups/20260813T190400Z");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("quick-task.toml"), "already\n").unwrap();
  }
  let stamp = [("TOOLU_TIMESTAMP", "20260813T190400Z")];
  let bun = script(&left, Home::Codex, &stamp, &["install"]).unwrap();
  let native = toolu(&right, Home::Codex, &stamp, &["install"]).unwrap();
  compare(&left, &right, &bun, &native).unwrap();
}
