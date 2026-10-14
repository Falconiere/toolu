//! Shared test fixtures: a sandbox directory with stub host CLIs on `PATH`. A
//! stub stands in for `claude` or `codex` only at the process boundary, to
//! inject the failures and listings a real host cannot be made to produce.

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use tempfile::TempDir;
use toolu_runtime::env::Env;

use crate::catalog::{Catalog, load};
use crate::fail::{Fail, Res};
use crate::prompt::{Ask, Choice};

/// A temporary directory with a `bin/` for stub hosts and a `home/`.
pub(crate) struct Sandbox {
  dir: TempDir,
}

impl Sandbox {
  /// A fresh sandbox.
  pub(crate) fn new() -> Sandbox {
    let dir = tempfile::Builder::new()
      .prefix("toolu-plugins-")
      .tempdir()
      .expect("tempdir");
    std::fs::create_dir_all(dir.path().join("bin")).expect("bin");
    std::fs::create_dir_all(dir.path().join("home")).expect("home");
    Sandbox { dir }
  }

  /// The sandbox root, resolved (macOS temp dirs sit behind `/private`).
  pub(crate) fn root(&self) -> PathBuf {
    std::fs::canonicalize(self.dir.path()).expect("canonical")
  }

  /// `root/rel`.
  pub(crate) fn path(&self, rel: &str) -> PathBuf {
    self.root().join(rel)
  }

  /// Writes `text` at `root/rel`, creating directories.
  pub(crate) fn write(&self, rel: &str, text: &str) -> PathBuf {
    let path = self.path(rel);
    if let Some(dir) = path.parent() {
      std::fs::create_dir_all(dir).expect("parent");
    }
    std::fs::write(&path, text).expect("write");
    path
  }

  /// An executable `bin/<name>` running `body` under `/bin/sh`. It is written
  /// beside its name and renamed into place, so no test's spawn can inherit a
  /// descriptor open for writing on the inode another test executes (`ETXTBSY`).
  pub(crate) fn stub(&self, name: &str, body: &str) {
    let temp = self.write(&format!("bin/.{name}.tmp"), &format!("#!/bin/sh\n{body}\n"));
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    std::fs::rename(&temp, self.path(&format!("bin/{name}"))).expect("rename");
  }

  /// A host stub that logs its argv to `<name>.log`, prints the listings in
  /// `<name>-installed.json` and `<name>-available.json` (an empty listing
  /// when absent), and fails with `boom: <argv>` when argv matches `fail`.
  pub(crate) fn host(&self, name: &str, empty: &str, fail: &str) {
    let root = self.root();
    let root = root.display();
    let body = format!(
      "echo \"$*\" >> {root}/{name}.log\n\
       case \"$*\" in\n\
       \"plugin list --json\") cat {root}/{name}-installed.json 2>/dev/null || echo '{empty}' ;;\n\
       \"plugin list --available --json\") cat {root}/{name}-available.json 2>/dev/null || echo '{empty}' ;;\n\
       {fail}) echo \"boom: $*\" >&2; exit 1 ;;\n\
       *) exit 0 ;;\n\
       esac"
    );
    self.stub(name, &body);
  }

  /// The argv lines a host stub logged.
  pub(crate) fn log(&self, name: &str) -> Vec<String> {
    std::fs::read_to_string(self.path(&format!("{name}.log")))
      .unwrap_or_default()
      .lines()
      .map(str::to_owned)
      .collect()
  }

  /// An environment whose `PATH` is the stub `bin/` plus the system tools.
  pub(crate) fn env(&self) -> Env {
    let bin = self.path("bin");
    Env::from_pairs([
      ("PATH", format!("{}:/usr/bin:/bin", bin.display())),
      ("HOME", self.path("home").display().to_string()),
      (
        "XDG_CONFIG_HOME",
        self.path("home/.config").display().to_string(),
      ),
    ])
  }
}

/// The catalog this binary was built with.
pub(crate) fn catalog() -> Catalog {
  load(None).expect("built-in catalog")
}

/// `names` as owned strings.
pub(crate) fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(|name| (*name).to_owned()).collect()
}

/// Answers every prompt with fixed indices, as a person at the terminal would;
/// no index left means the person cancelled.
pub(crate) struct Answer(pub(crate) Vec<usize>);

impl Ask for Answer {
  fn many(&mut self, _message: &str, _options: &[Choice]) -> Res<Vec<usize>> {
    Ok(self.0.clone())
  }

  fn one(&mut self, _message: &str, _options: &[Choice]) -> Res<usize> {
    self
      .0
      .first()
      .copied()
      .ok_or_else(|| Fail::failed("cancelled"))
  }
}
