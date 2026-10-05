//! Temp workspaces of named crates for `cargo xtask check-layers`, carrying this
//! repository's layer table and rules.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fallible helper result.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const DATA_DIR: &str = "tooling/conventions/guardrails/rust";

/// One crate: its directory, package name, dependency lines and whether it is a binary.
pub(crate) struct Crate {
  pub(crate) dir: &'static str,
  name: &'static str,
  pub(crate) deps: Vec<String>,
  pub(crate) dev_deps: Vec<String>,
  pub(crate) bin: bool,
}

/// A library crate at `dir` named `name`.
pub(crate) fn krate(dir: &'static str, name: &'static str) -> Crate {
  Crate {
    dir,
    name,
    deps: Vec::new(),
    dev_deps: Vec::new(),
    bin: false,
  }
}

impl Crate {
  /// Depend on `name` at `dir`.
  pub(crate) fn dep(mut self, name: &str, dir: &str) -> Self {
    self
      .deps
      .push(format!("{name} = {{ path = \"{}\" }}", up(self.dir, dir)));
    self
  }
}

/// `to` relative to `from`, both relative to the workspace root.
pub(crate) fn up(from: &str, to: &str) -> String {
  let depth = Path::new(from).components().count();
  format!("{}{to}", "../".repeat(depth))
}

/// A temp workspace.
pub(crate) struct Layered {
  _temp: tempfile::TempDir,
  /// The workspace root.
  pub(crate) root: PathBuf,
}

/// A workspace with `members` listed and `extra` crates written but not listed.
pub(crate) fn workspace(members: &[Crate], extra: &[Crate]) -> Res<Layered> {
  let temp = tempfile::tempdir()?;
  let root = fs::canonicalize(temp.path())?;
  let list: Vec<String> = members.iter().map(|c| format!("\"{}\"", c.dir)).collect();
  fs::write(
    root.join("Cargo.toml"),
    format!(
      "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
      list.join(", ")
    ),
  )?;
  for krate in members.iter().chain(extra) {
    write_crate(&root, krate)?;
  }
  fs::create_dir_all(root.join(DATA_DIR))?;
  for file in ["layers.json", "rules.json"] {
    fs::copy(
      Path::new(REPO).join(DATA_DIR).join(file),
      root.join(DATA_DIR).join(file),
    )?;
  }
  Ok(Layered { _temp: temp, root })
}

fn write_crate(root: &Path, krate: &Crate) -> Res<()> {
  let dir = root.join(krate.dir);
  fs::create_dir_all(dir.join("src"))?;
  let manifest = format!(
    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{}\n\n[dev-dependencies]\n{}\n",
    krate.name,
    krate.deps.join("\n"),
    krate.dev_deps.join("\n")
  );
  fs::write(dir.join("Cargo.toml"), manifest)?;
  let (file, body) = if krate.bin {
    ("main.rs", "fn main() {}\n")
  } else {
    ("lib.rs", "")
  };
  fs::write(dir.join("src").join(file), body)?;
  Ok(())
}

/// `xtask <args>`.
pub(crate) fn xtask(args: &[&str]) -> Res<Output> {
  Ok(
    Command::new(env!("CARGO_BIN_EXE_xtask"))
      .args(args)
      .output()?,
  )
}

/// `xtask check-layers --root <workspace>`.
pub(crate) fn check(layered: &Layered) -> Res<Output> {
  let root = layered.root.to_str().ok_or("fixture path is not UTF-8")?;
  xtask(&["check-layers", "--root", root])
}

/// Standard error as text.
pub(crate) fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Standard output as text.
pub(crate) fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}
