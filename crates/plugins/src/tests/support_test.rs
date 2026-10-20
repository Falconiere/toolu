//! Shared test fixtures: a sandbox directory with stub host CLIs on `PATH`. A
//! stub stands in for `claude` or `codex` only at the process boundary, to
//! inject the failures and listings a real host cannot be made to produce.

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;

use tempfile::TempDir;
use toolu_runtime::env::Env;

use crate::args::Approval;
use crate::catalog::{Catalog, load};
use crate::fail::{Fail, Res};
use crate::host::adapter::Adapter;
use crate::host::checkout::Checkout;
use crate::host::run::run;
use crate::prompt::{Ask, Choice};
use crate::verbs::Driver;

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

  /// A `muse` stub that logs its argv to `muse.log`, prints `muse-installed.json`,
  /// `muse-available.json`, `muse-marketplaces.json` and `muse-inspect-<name>.json`
  /// (an empty document when absent), and fails with `boom: <argv>` when argv
  /// matches `fail`.
  pub(crate) fn muse(&self, fail: &str) {
    let root = self.root();
    let root = root.display();
    let body = format!(
      "echo \"$*\" >> {root}/muse.log\n\
       case \"$*\" in\n\
       {fail}) echo \"boom: $*\" >&2; exit 1 ;;\n\
       \"plugins list --json\") cat {root}/muse-installed.json 2>/dev/null || echo '{{\"plugins\":[]}}' ;;\n\
       \"plugins list --available --json\") cat {root}/muse-available.json 2>/dev/null || echo '{{\"available\":[]}}' ;;\n\
       \"plugins marketplace list --json\") cat {root}/muse-marketplaces.json 2>/dev/null || echo '{{\"marketplaces\":[]}}' ;;\n\
       \"plugins inspect \"*) cat {root}/muse-inspect-$3.json 2>/dev/null || echo '{{\"runtime_capabilities\":[]}}' ;;\n\
       *) exit 0 ;;\n\
       esac"
    );
    self.stub("muse", &body);
  }

  /// Writes the captured Muse case `case` at `rel`, its `/home/user` paths
  /// moved into this sandbox's home.
  pub(crate) fn capture(&self, case: &str, rel: &str) {
    let home = self.path("home").display().to_string();
    self.write(rel, &muse_output(case).replace("/home/user", &home));
  }

  /// Runs `git <args>` in the sandbox, which must succeed.
  pub(crate) fn git(&self, words: &[&str]) {
    let argv: Vec<String> = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
      .iter()
      .chain(words)
      .map(|word| (*word).to_owned())
      .collect();
    let ran = run(&argv, &self.env(), std::time::Duration::from_secs(60));
    assert!(ran.ok(), "{argv:?}: {}", ran.stderr);
  }

  /// A toolu origin repository at `origin/`, one commit per tag in `tags`, each
  /// carrying the catalog Muse reads; returns its `file://` URL.
  pub(crate) fn origin(&self, tags: &[&str]) -> String {
    let dir = self.path("origin").display().to_string();
    self.git(&["init", "--quiet", &dir]);
    for tag in tags {
      self.write(
        "origin/.agents/plugins/marketplace.json",
        &format!("{{\"tag\":\"{tag}\"}}"),
      );
      self.git(&["-C", &dir, "add", "--all"]);
      self.git(&["-C", &dir, "commit", "--quiet", "-m", tag]);
      self.git(&["-C", &dir, "tag", tag]);
    }
    format!("file://{dir}")
  }

  /// A Muse driver whose checkout follows `version` from `origin`.
  pub(crate) fn muse_driver(&self, version: &str, origin: &str, approval: Approval) -> Driver {
    Driver {
      adapter: Adapter::Muse,
      env: self.env(),
      timeout: std::time::Duration::from_secs(60),
      checkout: Some(Checkout::locate(&self.env(), version, origin).expect("checkout")),
      approval,
    }
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

/// The captured Muse 1.4.11 output of the case `name` in
/// `fixtures/plugins/muse.json`: its stdout, re-serialized.
pub(crate) fn muse_output(name: &str) -> String {
  let path =
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/plugins/muse.json");
  let text = std::fs::read_to_string(path).expect("fixtures/plugins/muse.json");
  let fixture: serde_json::Value = serde_json::from_str(&text).expect("fixture JSON");
  let case = fixture["cases"]
    .as_array()
    .expect("cases")
    .iter()
    .find(|case| case["name"] == name)
    .unwrap_or_else(|| panic!("no Muse case {name}"));
  case["stdout"].to_string()
}
