//! `toolu setup agents` against golden transcripts (`fixtures/setup_agents_*.snap`).
//! They were recorded when the native verb still matched the retired
//! `setup.ts` byte for byte, apart from the program name (#445, #440).

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");
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

/// One run as the transcript records it: exit code, stdout and stderr, the root as `ROOT`.
fn render(out: &Out, side: &Side) -> String {
  let root = side.root.display().to_string();
  format!(
    "exit {}\n--- stdout\n{}--- stderr\n{}",
    out.code,
    out.stdout.replace(&root, "ROOT"),
    out.stderr.replace(&root, "ROOT")
  )
}

/// Runs of one scenario, compared as one snapshot.
struct Transcript {
  side: Side,
  runs: Vec<String>,
}

impl Transcript {
  fn new() -> Result<Self, String> {
    Ok(Self {
      side: sandbox()?,
      runs: Vec::new(),
    })
  }

  fn run(&mut self, home: Home, extra: &[(&str, &str)], args: &[&str]) -> Result<(), String> {
    let out = toolu(&self.side, home, extra, args)?;
    let line = format!(
      "$ toolu setup agents {}\n{}",
      args.join(" "),
      render(&out, &self.side)
    );
    self.runs.push(line);
    Ok(())
  }

  fn file(&mut self, rel: &str) -> Result<(), String> {
    let text = fs::read_to_string(self.side.root.join(rel)).map_err(|err| err.to_string())?;
    self.runs.push(format!("$ cat {rel}\n{text}"));
    Ok(())
  }

  fn check(&self, name: &str) {
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path("fixtures");
    settings.set_prepend_module_to_snapshot(false);
    let text = self.runs.join("\n");
    settings.bind(|| insta::assert_snapshot!(name, text));
  }
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
fn preview_install_and_refusal() {
  let mut transcript = Transcript::new().unwrap();
  transcript.run(Home::Fallback, &[], &["preview"]).unwrap();
  transcript.run(Home::Codex, &[], &["install"]).unwrap();
  transcript
    .run(Home::Codex, &[], &["install", "--now"])
    .unwrap();
  transcript.run(Home::Codex, &[], &["deploy"]).unwrap();
  transcript.run(Home::Unset, &[], &["preview"]).unwrap();
  transcript.check("setup_agents_preview");
}

#[test]
fn a_second_install_and_a_managed_update() {
  let mut t = Transcript::new().unwrap();
  t.run(Home::Codex, &[], &["install"]).unwrap();
  t.run(Home::Codex, &[], &["install"]).unwrap();
  let path = codex_file(&t.side, "quick-task");
  let text = fs::read_to_string(&path).unwrap();
  let low = text.replace(
    "model_reasoning_effort = \"medium\"",
    "model_reasoning_effort = \"low\"",
  );
  fs::write(&path, low).unwrap();
  t.run(
    Home::Codex,
    &[("TOOLU_TIMESTAMP", "20260813T190000Z")],
    &["install"],
  )
  .unwrap();
  t.file("codex/agents/.toolu-backups/20260813T190000Z/quick-task.toml")
    .unwrap();
  t.check("setup_agents_update");
}

#[test]
fn conflict_remove_and_invalid_templates() {
  let mut t = Transcript::new().unwrap();
  write_personal(&t.side).unwrap();
  t.run(Home::Codex, &[], &["preview"]).unwrap();
  t.run(Home::Codex, &[], &["install"]).unwrap();
  let stamp = [("TOOLU_TIMESTAMP", "20260813T190100Z")];
  t.run(Home::Codex, &stamp, &["install", "--force"]).unwrap();
  t.run(Home::Codex, &[], &["remove"]).unwrap();
  t.run(
    Home::Codex,
    &[("TOOLU_TIMESTAMP", "20260813T190200Z")],
    &["remove", "--yes"],
  )
  .unwrap();
  let bad = copy_bad_templates(&t.side).unwrap().display().to_string();
  t.run(
    Home::Codex,
    &[("TOOLU_AGENT_TEMPLATE_DIR", &bad)],
    &["install"],
  )
  .unwrap();
  t.check("setup_agents_conflict");
}

#[test]
fn forced_remove_bad_timestamp_and_existing_backup() {
  let mut t = Transcript::new().unwrap();
  write_personal(&t.side).unwrap();
  let force = [("TOOLU_TIMESTAMP", "20260813T190300Z")];
  t.run(Home::Codex, &force, &["remove", "--yes", "--force"])
    .unwrap();
  t.run(Home::Codex, &[], &["install"]).unwrap();
  t.run(
    Home::Codex,
    &[("TOOLU_TIMESTAMP", "../x")],
    &["remove", "--yes"],
  )
  .unwrap();
  let path = codex_file(&t.side, "quick-task");
  let text = fs::read_to_string(&path).unwrap();
  let low = text.replace(
    "model_reasoning_effort = \"medium\"",
    "model_reasoning_effort = \"low\"",
  );
  fs::write(&path, low).unwrap();
  let dir = t
    .side
    .root
    .join("codex/agents/.toolu-backups/20260813T190400Z");
  fs::create_dir_all(&dir).unwrap();
  fs::write(dir.join("quick-task.toml"), "already\n").unwrap();
  t.run(
    Home::Codex,
    &[("TOOLU_TIMESTAMP", "20260813T190400Z")],
    &["install"],
  )
  .unwrap();
  t.check("setup_agents_backup");
}
