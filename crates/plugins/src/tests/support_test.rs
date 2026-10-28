//! Shared test fixtures: a sandbox directory with stub host CLIs on `PATH`. A
//! stub stands in for `claude` or `codex` only at the process boundary, to
//! inject the failures and listings a real host cannot be made to produce.

use std::fmt::Write as _;
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

  /// A `claude` stub replaying captured cases: it logs its argv to `claude.log`;
  /// when argv is a key of `cases`, it prints that `claude.json` case's stdout and
  /// stderr and exits with its code; any other argv exits 0 silently.
  pub(crate) fn replay(&self, cases: &[(&str, &str)]) {
    let root = self.root();
    let root = root.display();
    let mut arms = String::new();
    for (index, (argv, name)) in cases.iter().enumerate() {
      let case = claude_case(name);
      self.write(&format!("replay-{index}.out"), &case["stdout"].to_string());
      self.write(
        &format!("replay-{index}.err"),
        case["stderr"].as_str().unwrap_or_default(),
      );
      let code = case["exit"].as_i64().expect("exit");
      writeln!(
        arms,
        "\"{argv}\") cat {root}/replay-{index}.out; cat {root}/replay-{index}.err >&2; exit {code} ;;"
      )
      .expect("write to a String");
    }
    let body =
      format!("echo \"$*\" >> {root}/claude.log\ncase \"$*\" in\n{arms}*) exit 0 ;;\nesac");
    self.stub("claude", &body);
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

  /// A toolu origin repository whose `plugins/` holds real copies of the
  /// `toolu` and `ast-grep` plugins (the cursor installer's source); commits and
  /// tags `tag`, setting every plugin's version to `version` when given.
  pub(crate) fn local_origin(&self, tag: &str, version: Option<&str>) -> String {
    let dir = self.path("origin");
    if !dir.join(".git").is_dir() {
      self.git(&["init", "--quiet", &dir.display().to_string()]);
      for name in ["toolu", "ast-grep"] {
        let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
          .join("../../plugins")
          .join(name);
        copy_tree(&from, &dir.join("plugins").join(name));
      }
    }
    if let Some(version) = version {
      for name in ["toolu", "ast-grep"] {
        let manifest = dir
          .join("plugins")
          .join(name)
          .join(".claude-plugin/plugin.json");
        let json: serde_json::Value =
          serde_json::from_str(&std::fs::read_to_string(&manifest).expect("manifest"))
            .expect("json");
        let mut json = json;
        json["version"] = serde_json::Value::String(version.to_owned());
        std::fs::write(&manifest, json.to_string()).expect("write manifest");
      }
    }
    self.write(
      "origin/.agents/plugins/marketplace.json",
      &format!("{{\"tag\":\"{tag}\"}}"),
    );
    let at = dir.display().to_string();
    self.git(&["-C", &at, "add", "--all"]);
    self.git(&["-C", &at, "commit", "--quiet", "-m", tag]);
    self.git(&["-C", &at, "tag", tag]);
    format!("file://{at}")
  }

  /// A Muse driver whose checkout follows `version` from `origin`.
  pub(crate) fn muse_driver(&self, version: &str, origin: &str, approval: Approval) -> Driver {
    Driver {
      adapter: Adapter::Muse,
      env: self.env(),
      timeout: std::time::Duration::from_secs(60),
      checkout: Some(Checkout::locate(&self.env(), version, origin).expect("checkout")),
      pin: None,
      approval,
      progress: None,
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

/// The case `name` of the captured host output in `fixtures/plugins/<host>.json`.
fn fixture_case(host: &str, name: &str) -> serde_json::Value {
  let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join(format!("../../fixtures/plugins/{host}.json"));
  let text = std::fs::read_to_string(path).expect("fixture file");
  let fixture: serde_json::Value = serde_json::from_str(&text).expect("fixture JSON");
  fixture["cases"]
    .as_array()
    .expect("cases")
    .iter()
    .find(|case| case["name"] == name)
    .cloned()
    .unwrap_or_else(|| panic!("no {host} case {name}"))
}

/// The captured Muse 1.4.11 output of the case `name` in
/// `fixtures/plugins/muse.json`: its stdout, re-serialized.
pub(crate) fn muse_output(name: &str) -> String {
  fixture_case("muse", name)["stdout"].to_string()
}

/// The captured Claude Code 2.1.311 case `name` in `fixtures/plugins/claude.json`:
/// `argv`, `exit`, `stdout` (a JSON value) and `stderr`.
pub(crate) fn claude_case(name: &str) -> serde_json::Value {
  fixture_case("claude", name)
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
  std::fs::create_dir_all(to).expect("dir");
  for entry in std::fs::read_dir(from).expect("read") {
    let entry = entry.expect("entry");
    let target = to.join(entry.file_name());
    if entry.path().is_dir() {
      copy_tree(&entry.path(), &target);
    } else {
      std::fs::copy(entry.path(), target).expect("copy");
    }
  }
}

/// A run context over the sandbox's environment, drawn in `style`.
pub(crate) fn context<'a>(
  sb: &Sandbox,
  host: Option<toolu_protocol::host::Host>,
  ask: Option<&'a mut dyn Ask>,
  style: toolu_runtime::term::Style,
) -> crate::dispatch::Context<'a> {
  crate::dispatch::Context {
    env: sb.env(),
    host,
    ask,
    timeout: std::time::Duration::from_secs(10),
    version: "7.11.0",
    origin: String::new(),
    style,
    started: std::time::Instant::now(),
    progress: None,
  }
}

/// A writer the test reads back.
#[derive(Clone, Default)]
pub(crate) struct Screen(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Screen {
  fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
    self
      .0
      .lock()
      .map_err(|_poisoned| std::io::Error::other("poisoned"))?
      .extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

impl Screen {
  /// Everything written so far.
  pub(crate) fn text(&self) -> String {
    String::from_utf8_lossy(&self.0.lock().expect("screen")).into_owned()
  }
}

impl Screen {
  /// A Mono progress drawn on this screen, as a driver carries it.
  pub(crate) fn progress(&self) -> std::sync::Arc<crate::progress::Progress> {
    let progress = crate::progress::Progress::to_writer(
      Box::new(self.clone()),
      toolu_runtime::term::Style::Mono,
    );
    std::sync::Arc::new(progress)
  }
}

/// A request for `verb` on `plugins`, every flag off.
pub(crate) fn request(verb: crate::args::Verb, plugins: &[&str]) -> crate::args::Request {
  crate::args::Request {
    verb,
    names: names(plugins),
    scope: None,
    config: None,
    yes: false,
    dry_run: false,
    no_input: false,
    catalog: None,
    approval: Approval::Report,
  }
}
