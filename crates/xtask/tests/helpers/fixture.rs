//! Materialise a quality-bar fixture (`fixtures/guardrails/rust/<rule>/<case>`)
//! as a real git workspace carrying this repository's gate data, and run
//! `xtask gate` on it.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fallible helper result.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

/// This repository.
pub(crate) const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

const FIXTURES: &str = "fixtures/guardrails/rust";

/// The AWS documentation example key id, in parts so no committed file holds it.
const FAKE_AWS_KEY: [&str; 3] = ["AKIA", "IOSFODNN7", "EXAMPLE"];

/// Gate data copied verbatim from this repository into every fixture.
const GATE_FILES: &[&str] = &[
  "clippy.toml",
  "rustfmt.toml",
  "deny.toml",
  "rust-toolchain.toml",
  ".claude/toolu.config.json",
  "tooling/conventions/guardrails/rust/rules.json",
  "tooling/conventions/guardrails/rust/jscpd.json",
];

/// Variables a coverage run sets that must not reach the fixture's own builds.
const BUILD_ENV: &[&str] = &[
  "RUSTFLAGS",
  "CARGO_ENCODED_RUSTFLAGS",
  "CARGO_BUILD_RUSTFLAGS",
  "RUSTDOCFLAGS",
  "CARGO_ENCODED_RUSTDOCFLAGS",
  "RUSTC_WRAPPER",
  "RUSTC_WORKSPACE_WRAPPER",
  "CARGO_TARGET_DIR",
  "CARGO_BUILD_TARGET_DIR",
  "CARGO_INCREMENTAL",
];

/// What a fixture case expects (`expect.txt`).
#[derive(Debug, Default)]
pub(crate) struct Expect {
  /// Gate steps to run (`step a b`).
  pub(crate) steps: Vec<String>,
  /// `--title` for check-gate-change (`title …`).
  pub(crate) title: Option<String>,
  /// The exit code (`exit n`, default 0).
  pub(crate) exit: i32,
  /// Substrings the combined output must hold (`contains …`, repeatable).
  pub(crate) contains: Vec<String>,
}

/// A materialised fixture.
pub(crate) struct Fixture {
  _dir: tempfile::TempDir,
  /// The workspace root.
  pub(crate) root: PathBuf,
  /// What the case expects.
  pub(crate) expect: Expect,
}

fn parse_expect(text: &str) -> Res<Expect> {
  let mut expect = Expect::default();
  for line in text.lines().filter(|line| !line.trim().is_empty()) {
    let (key, value) = line
      .split_once(' ')
      .ok_or_else(|| format!("bad expect line: {line}"))?;
    match key {
      "step" => expect.steps = value.split_whitespace().map(str::to_owned).collect(),
      "title" => expect.title = Some(value.to_owned()),
      "exit" => expect.exit = value.parse()?,
      "contains" => expect.contains.push(value.to_owned()),
      _ => return Err(format!("unknown expect key {key}").into()),
    }
  }
  Ok(expect)
}

/// Copy `from` into `to`, dropping `.fixture` suffixes, filling the fake key
/// and skipping `expect.txt` and the `base`/`head` overlays of gate-change cases.
fn overlay(from: &Path, to: &Path) -> Res<()> {
  for entry in fs::read_dir(from)? {
    let entry = entry?;
    let name = entry.file_name().to_string_lossy().into_owned();
    let source = entry.path();
    if source.is_dir() {
      if from.join("expect.txt").is_file() && (name == "base" || name == "head") {
        continue;
      }
      fs::create_dir_all(to.join(&name))?;
      overlay(&source, &to.join(&name))?;
      continue;
    }
    if name == "expect.txt" {
      continue;
    }
    let target = to.join(name.strip_suffix(".fixture").unwrap_or(&name));
    let text = fs::read_to_string(&source)?.replace("{{FAKE_AWS_KEY}}", &FAKE_AWS_KEY.concat());
    fs::write(target, text)?;
  }
  Ok(())
}

/// Write this repository's gate data into `root`.
fn install_gate_data(root: &Path) -> Res<()> {
  let repo = Path::new(REPO);
  for file in GATE_FILES {
    fs::create_dir_all(root.join(file).parent().ok_or("no parent")?)?;
    fs::copy(repo.join(file), root.join(file))?;
  }
  let manifest = fs::read_to_string(repo.join("Cargo.toml"))?;
  let start = manifest
    .find("[workspace.lints.rust]")
    .ok_or("no [workspace.lints.rust]")?;
  let end = manifest
    .find("# Hooks wrap their main")
    .ok_or("no end of lints")?;
  let lints = manifest.get(start..end).ok_or("lints out of range")?;
  let fixture = fs::read_to_string(root.join("Cargo.toml"))?;
  fs::write(root.join("Cargo.toml"), format!("{fixture}\n{lints}"))?;
  Ok(())
}

fn git(root: &Path, args: &[&str]) -> Res<()> {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(args)
    .output()?;
  if !output.status.success() {
    return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)).into());
  }
  Ok(())
}

/// Build the fixture `rule/case`: base, gate data, the case overlay (and for a
/// gate-change case its `base/` overlay committed before its `head/` overlay).
pub(crate) fn fixture(rule: &str, case: &str) -> Res<Fixture> {
  let dir = tempfile::tempdir()?;
  let root = fs::canonicalize(dir.path())?;
  let fixtures = Path::new(REPO).join(FIXTURES);
  let case_dir = fixtures.join(rule).join(case);
  overlay(&fixtures.join("base"), &root)?;
  install_gate_data(&root)?;
  overlay(&case_dir, &root)?;
  if case_dir.join("base").is_dir() {
    overlay(&case_dir.join("base"), &root)?;
  }
  git(&root, &["init", "-q", "-b", "main"])?;
  git(&root, &["add", "-A"])?;
  git(
    &root,
    &[
      "-c",
      "user.name=fixture",
      "-c",
      "user.email=fixture@example.com",
      "commit",
      "-qm",
      "base",
    ],
  )?;
  if case_dir.join("head").is_dir() {
    overlay(&case_dir.join("head"), &root)?;
  }
  let expect = parse_expect(&fs::read_to_string(case_dir.join("expect.txt"))?)?;
  Ok(Fixture {
    _dir: dir,
    root,
    expect,
  })
}

/// Run `xtask <args>` in the fixture with a clean build environment.
pub(crate) fn xtask(fixture: &Fixture, args: &[&str]) -> Res<Output> {
  let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
  command
    .args(args)
    .arg("--root")
    .arg(&fixture.root)
    .current_dir(&fixture.root);
  for name in BUILD_ENV {
    command.env_remove(name);
  }
  let node_bin = Path::new(REPO).join("node_modules/.bin");
  let path = std::env::var_os("PATH").unwrap_or_default();
  let mut paths = vec![node_bin];
  paths.extend(std::env::split_paths(&path));
  command
    .env("PATH", std::env::join_paths(paths)?)
    .env("CARGO_TARGET_DIR", fixture.root.join("target"));
  Ok(command.output()?)
}

/// Run the case's gate steps and check its exit code and output.
pub(crate) fn check(rule: &str, case: &str) -> Res<()> {
  let fixture = fixture(rule, case)?;
  let mut args = vec!["gate"];
  for step in &fixture.expect.steps {
    args.extend(["--only", step.as_str()]);
  }
  if let Some(title) = &fixture.expect.title {
    args.extend(["--title", title.as_str()]);
  }
  if fixture
    .expect
    .steps
    .iter()
    .any(|step| step == "gate-change")
  {
    args.extend(["--base", "HEAD"]);
  }
  let output = xtask(&fixture, &args)?;
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  if output.status.code() != Some(fixture.expect.exit) {
    return Err(
      format!(
        "{rule}/{case}: exit {:?}, expected {}\n{text}",
        output.status.code(),
        fixture.expect.exit
      )
      .into(),
    );
  }
  if let Some(missing) = fixture
    .expect
    .contains
    .iter()
    .find(|want| !text.contains(want.as_str()))
  {
    return Err(format!("{rule}/{case}: output lacks `{missing}`\n{text}").into());
  }
  Ok(())
}
